//! Durable project payloads, scoped by project and (for chat) authenticated user.
use anyhow::Result;
use diraigent_types::project_content::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

const MAX_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone)]
pub struct ContentStore {
    connection: Arc<Mutex<Connection>>,
    identity: Uuid,
    _lock: Arc<File>,
}
impl ContentStore {
    pub fn id(&self) -> Uuid {
        self.identity
    }
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(directory.join("project-content.lock"))?;
        // A second process must not reset a running conversation's reservation.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            anyhow::bail!("project content store is already in use");
        }
        let database = directory.join("project-content.db");
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(&database)?;
        std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600))?;
        let conn = Connection::open(database)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
            CREATE TABLE IF NOT EXISTS payload (
                project TEXT NOT NULL, kind TEXT NOT NULL, id TEXT NOT NULL,
                value TEXT NOT NULL, PRIMARY KEY(project,kind,id));
            CREATE TABLE IF NOT EXISTS conversation (
                project TEXT NOT NULL, user TEXT NOT NULL, revision INTEGER NOT NULL,
                messages TEXT NOT NULL, busy INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(project,user));
            UPDATE conversation SET busy=0;",
        )?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS store_identity (singleton INTEGER PRIMARY KEY CHECK(singleton=1), id TEXT NOT NULL);")?;
        conn.execute(
            "INSERT OR IGNORE INTO store_identity VALUES (1,?)",
            [Uuid::new_v4().to_string()],
        )?;
        let id: String =
            conn.query_row("SELECT id FROM store_identity WHERE singleton=1", [], |r| {
                r.get(0)
            })?;
        Ok(Self {
            connection: Arc::new(Mutex::new(conn)),
            identity: id.parse()?,
            _lock: Arc::new(lock),
        })
    }
    fn history(conn: &Connection, project: Uuid, user: Uuid) -> Result<ChatHistory> {
        let row: Option<(i64, String, bool)> = conn
            .query_row(
                "SELECT revision,messages,busy FROM conversation WHERE project=? AND user=?",
                params![project.to_string(), user.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        match row {
            Some((revision, messages, busy)) => Ok(ChatHistory {
                enabled: true,
                revision,
                messages: serde_json::from_str(&messages)?,
                busy,
            }),
            None => Ok(ChatHistory::default()),
        }
    }
    pub fn request(&self, project: Uuid, request: ContentRequest) -> ContentResult {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| ContentError::Unavailable)?;
        match request {
            ContentRequest::Probe => Ok(json!({"protocol":1,"store_id":self.id()})),
            ContentRequest::Get { kind, id } => {
                let value: Option<String> = conn
                    .query_row(
                        "SELECT value FROM payload WHERE project=? AND kind=? AND id=?",
                        params![project.to_string(), kind.as_str(), id.to_string()],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|_| ContentError::Unavailable)?;
                serde_json::from_str(&value.ok_or(ContentError::NotFound)?)
                    .map_err(|_| ContentError::Unavailable)
            }
            ContentRequest::Put { kind, id, value } => {
                let text = serde_json::to_string(&value).map_err(|_| ContentError::Invalid)?;
                if text.len() > MAX_BYTES {
                    return Err(ContentError::Invalid);
                }
                // Immutable objects make retries safe and prevent silent overwrite.
                let tx = conn.transaction().map_err(|_| ContentError::Unavailable)?;
                tx.execute(
                    "INSERT OR IGNORE INTO payload VALUES (?,?,?,?)",
                    params![project.to_string(), kind.as_str(), id.to_string(), text],
                )
                .map_err(|_| ContentError::Unavailable)?;
                let stored: String = tx
                    .query_row(
                        "SELECT value FROM payload WHERE project=? AND kind=? AND id=?",
                        params![project.to_string(), kind.as_str(), id.to_string()],
                        |r| r.get(0),
                    )
                    .map_err(|_| ContentError::Unavailable)?;
                if stored != text {
                    return Err(ContentError::Conflict);
                }
                tx.commit().map_err(|_| ContentError::Unavailable)?;
                Ok(json!({"stored":true}))
            }
            ContentRequest::History { user_id } => serde_json::to_value(
                Self::history(&conn, project, user_id).map_err(|_| ContentError::Unavailable)?,
            )
            .map_err(|_| ContentError::Unavailable),
            ContentRequest::ClearHistory { user_id, revision } => {
                let h = Self::history(&conn, project, user_id)
                    .map_err(|_| ContentError::Unavailable)?;
                if h.busy || h.revision != revision {
                    return Err(ContentError::Conflict);
                }
                conn.execute("INSERT INTO conversation VALUES (?,?,?,?,0) ON CONFLICT(project,user) DO UPDATE SET revision=excluded.revision,messages=excluded.messages,busy=0",
                    params![project.to_string(),user_id.to_string(),revision+1,"[]"]).map_err(|_|ContentError::Unavailable)?;
                Ok(json!({"enabled":true,"revision":revision+1,"messages":[],"busy":false}))
            }
        }
    }
    pub fn begin_chat(
        &self,
        project: Uuid,
        user: Uuid,
        revision: Option<i64>,
        messages: Vec<HistoryMessage>,
    ) -> Result<i64, ContentError> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| ContentError::Unavailable)?;
        let h = Self::history(&conn, project, user).map_err(|_| ContentError::Unavailable)?;
        if h.busy || revision != Some(h.revision) {
            return Err(ContentError::Conflict);
        }
        if messages.len() != h.messages.len() + 1
            || !messages.starts_with(&h.messages)
            || messages.last().is_none_or(|m| m.role != "user")
        {
            return Err(ContentError::Conflict);
        }
        let text = serde_json::to_string(&messages).map_err(|_| ContentError::Invalid)?;
        if text.len() > MAX_BYTES / 2 {
            return Err(ContentError::Invalid);
        }
        conn.execute("INSERT INTO conversation VALUES (?,?,?,?,1) ON CONFLICT(project,user) DO UPDATE SET revision=excluded.revision,messages=excluded.messages,busy=1",
            params![project.to_string(),user.to_string(),h.revision+1,text]).map_err(|_|ContentError::Unavailable)?;
        Ok(h.revision + 1)
    }
    pub fn finish_chat(
        &self,
        project: Uuid,
        user: Uuid,
        revision: i64,
        text: String,
    ) -> Result<()> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Content storage unavailable"))?;
        let mut h = Self::history(&conn, project, user)?;
        anyhow::ensure!(
            h.busy && h.revision == revision,
            "Conversation changed during execution"
        );
        if !text.is_empty() {
            h.messages.push(HistoryMessage {
                role: "assistant".into(),
                content: text,
            });
        }
        let text = serde_json::to_string(&h.messages)?;
        if text.len() > MAX_BYTES {
            h.messages.pop();
            h.messages.push(HistoryMessage {
                role: "assistant".into(),
                content: "[Response exceeded the conversation storage limit]".into(),
            });
        }
        let text = serde_json::to_string(&h.messages)?;
        conn.execute("UPDATE conversation SET messages=?,revision=revision+1,busy=0 WHERE project=? AND user=? AND revision=?",
            params![text,project.to_string(),user.to_string(),revision])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_store_identity_and_oversized_response_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let s = ContentStore::open(dir.path()).unwrap();
        let identity = s.id();
        assert!(ContentStore::open(dir.path()).is_err());
        assert_eq!(
            std::fs::metadata(dir.path().join("project-content.db"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let p = Uuid::new_v4();
        let u = Uuid::new_v4();
        let revision = s
            .begin_chat(
                p,
                u,
                Some(0),
                vec![HistoryMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
            )
            .unwrap();
        s.finish_chat(p, u, revision, "x".repeat(MAX_BYTES))
            .unwrap();
        let history = s
            .request(p, ContentRequest::History { user_id: u })
            .unwrap();
        assert_eq!(history["busy"], false);
        assert!(
            history["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("storage limit")
        );
        drop(s);
        assert_eq!(ContentStore::open(dir.path()).unwrap().id(), identity);
    }
    #[test]
    fn persistence_isolation_and_immutable_retries() {
        let dir = tempfile::tempdir().unwrap();
        let p = Uuid::new_v4();
        let other = Uuid::new_v4();
        let id = Uuid::new_v4();
        let put = ContentRequest::Put {
            kind: ContentKind::Diff,
            id,
            value: json!({"diff":"private"}),
        };
        {
            let s = ContentStore::open(dir.path()).unwrap();
            s.request(p, put.clone()).unwrap();
            s.request(p, put).unwrap();
            assert_eq!(
                s.request(
                    p,
                    ContentRequest::Put {
                        kind: ContentKind::Diff,
                        id,
                        value: json!("different")
                    }
                ),
                Err(ContentError::Conflict)
            );
        }
        let s = ContentStore::open(dir.path()).unwrap();
        assert_eq!(
            s.request(
                other,
                ContentRequest::Get {
                    kind: ContentKind::Diff,
                    id
                }
            ),
            Err(ContentError::NotFound)
        );
        assert_eq!(
            s.request(
                p,
                ContentRequest::Get {
                    kind: ContentKind::Diff,
                    id
                }
            )
            .unwrap()["diff"],
            "private"
        );
    }
    #[test]
    fn chat_cas_user_isolation_and_restart_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let p = Uuid::new_v4();
        let u = Uuid::new_v4();
        let s = ContentStore::open(dir.path()).unwrap();
        let m = vec![HistoryMessage {
            role: "user".into(),
            content: "hello".into(),
        }];
        let rev = s.begin_chat(p, u, Some(0), m.clone()).unwrap();
        assert_eq!(s.begin_chat(p, u, Some(0), m), Err(ContentError::Conflict));
        assert_eq!(
            s.request(
                p,
                ContentRequest::ClearHistory {
                    user_id: u,
                    revision: rev
                }
            ),
            Err(ContentError::Conflict)
        );
        s.finish_chat(p, u, rev, "answer".into()).unwrap();
        assert_eq!(
            s.request(
                p,
                ContentRequest::History {
                    user_id: Uuid::new_v4()
                }
            )
            .unwrap()["messages"],
            json!([])
        );
        let h: ChatHistory = serde_json::from_value(
            s.request(p, ContentRequest::History { user_id: u })
                .unwrap(),
        )
        .unwrap();
        assert_eq!(h.messages.len(), 2);
        let mut next = h.messages;
        next.push(HistoryMessage {
            role: "user".into(),
            content: "next".into(),
        });
        s.begin_chat(p, u, Some(h.revision), next).unwrap();
        drop(s);
        let s = ContentStore::open(dir.path()).unwrap();
        assert_eq!(
            s.request(p, ContentRequest::History { user_id: u })
                .unwrap()["busy"],
            false
        );
    }
}
