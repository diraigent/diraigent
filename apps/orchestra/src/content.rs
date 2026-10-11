//! Durable project payloads, scoped by project and (for chat) authenticated user.
use anyhow::Result;
use diraigent_types::project_content::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
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
            ContentRequest::Probe => Ok(json!({"protocol":2,"store_id":self.id()})),
            ContentRequest::RankFields {
                candidates,
                vector,
                limit,
            } => {
                if candidates.len() > 10_000 || vector.len() > 65_536 || limit > 100 {
                    return Err(ContentError::Invalid);
                }
                let mut scores = Vec::new();
                for (id, reference) in candidates {
                    let value = Self::field(&conn, project, &reference)?;
                    let embedding: Vec<f64> =
                        serde_json::from_value(value).map_err(|_| ContentError::Invalid)?;
                    if embedding.len() != vector.len() {
                        continue;
                    }
                    let dot: f64 = embedding.iter().zip(&vector).map(|(a, b)| a * b).sum();
                    let a: f64 = embedding.iter().map(|v| v * v).sum();
                    let b: f64 = vector.iter().map(|v| v * v).sum();
                    if a > 0.0 && b > 0.0 {
                        scores.push((id, dot / (a.sqrt() * b.sqrt())));
                    }
                }
                scores.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                Ok(json!(
                    scores
                        .into_iter()
                        .take(limit)
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>()
                ))
            }
            ContentRequest::SearchFields {
                candidates,
                query,
                limit,
                offset,
            } => {
                if candidates.len() > 10_000 || query.len() > 1024 || limit > 100 {
                    return Err(ContentError::Invalid);
                }
                let query = query.to_lowercase();
                let mut hits = Vec::new();
                for candidate in candidates {
                    let mut texts = Vec::new();
                    for field in candidate.fields {
                        let value = if let Some(reference) =
                            FieldRef::parse(&field).map_err(|_| ContentError::Invalid)?
                        {
                            Self::field(&conn, project, &reference)?
                        } else {
                            field
                        };
                        texts.push(
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string()),
                        );
                    }
                    let title = candidate.title.to_lowercase();
                    let body = texts
                        .iter()
                        .find(|text| text.to_lowercase().contains(&query));
                    if !title.contains(&query) && body.is_none() {
                        continue;
                    }
                    let relevance = if title == query {
                        1.0
                    } else if title.starts_with(&query) {
                        0.9
                    } else if title.contains(&query) {
                        0.7
                    } else {
                        0.3
                    };
                    hits.push(json!({"entity_type":candidate.entity_type,"id":candidate.id,"title":candidate.title,
                        "created_at":candidate.created_at,"relevance":relevance,"snippet":body.or(texts.first()).map(|s| s.chars().take(200).collect::<String>())}));
                }
                hits.sort_by(|a, b| {
                    b["relevance"]
                        .as_f64()
                        .partial_cmp(&a["relevance"].as_f64())
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b["created_at"].as_str().cmp(&a["created_at"].as_str()))
                        .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
                });
                let total = hits.len();
                Ok(
                    json!({"data":hits.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"total":total}),
                )
            }
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
    fn field(
        conn: &Connection,
        project: Uuid,
        reference: &FieldRef,
    ) -> std::result::Result<Value, ContentError> {
        use sha2::{Digest, Sha256};
        if reference.project != project {
            return Err(ContentError::Invalid);
        }
        let text: Option<String> = conn
            .query_row(
                "SELECT value FROM payload WHERE project=? AND kind='field' AND id=?",
                params![project.to_string(), reference.id.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ContentError::Unavailable)?;
        let value: Value = serde_json::from_str(&text.ok_or(ContentError::NotFound)?)
            .map_err(|_| ContentError::Unavailable)?;
        let bytes = serde_json::to_vec(&value).map_err(|_| ContentError::Unavailable)?;
        if bytes.len() != reference.bytes
            || format!("{:x}", Sha256::digest(bytes)) != reference.checksum
        {
            return Err(ContentError::Unavailable);
        }
        Ok(value)
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
    fn put_field(store: &ContentStore, project: Uuid, value: Value) -> FieldRef {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(&value).unwrap();
        let reference = FieldRef {
            project,
            id: Uuid::new_v4(),
            checksum: format!("{:x}", Sha256::digest(&bytes)),
            bytes: bytes.len(),
        };
        store
            .request(
                project,
                ContentRequest::Put {
                    kind: ContentKind::Field,
                    id: reference.id,
                    value,
                },
            )
            .unwrap();
        reference
    }

    #[test]
    fn field_search_and_ranking_survive_restart_and_reject_wrong_project_or_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let project = Uuid::new_v4();
        let id = Uuid::new_v4();
        let body;
        let aligned;
        let opposite;
        {
            let store = ContentStore::open(dir.path()).unwrap();
            body = put_field(
                &store,
                project,
                json!("秘密 needle in the worker's document"),
            );
            aligned = put_field(&store, project, json!([1.0, 0.0]));
            opposite = put_field(&store, project, json!([-1.0, 0.0]));
        }
        let store = ContentStore::open(dir.path()).unwrap();
        let search = ContentRequest::SearchFields {
            candidates: vec![ContentSearchCandidate {
                entity_type: "knowledge".into(),
                id,
                title: "Guide".into(),
                created_at: "2026-10-11T12:00:00Z".into(),
                fields: vec![body.encode(&json!(""))],
            }],
            query: "NEEDLE".into(),
            limit: 10,
            offset: 0,
        };
        let result = store.request(project, search.clone()).unwrap();
        assert_eq!(result["total"], 1);
        assert_eq!(result["data"][0]["id"], id.to_string());
        assert!(
            result["data"][0]["snippet"]
                .as_str()
                .unwrap()
                .contains("秘密")
        );
        assert_eq!(
            store.request(Uuid::new_v4(), search),
            Err(ContentError::Invalid)
        );
        let second = Uuid::new_v4();
        let ranked = store
            .request(
                project,
                ContentRequest::RankFields {
                    candidates: vec![(second, opposite), (id, aligned)],
                    vector: vec![1.0, 0.0],
                    limit: 1,
                },
            )
            .unwrap();
        assert_eq!(ranked, json!([id]));
        let mut corrupted = body;
        corrupted.checksum = "incorrect".into();
        assert_eq!(
            store.request(
                project,
                ContentRequest::SearchFields {
                    candidates: vec![ContentSearchCandidate {
                        entity_type: "knowledge".into(),
                        id,
                        title: "Guide".into(),
                        created_at: String::new(),
                        fields: vec![corrupted.encode(&json!(""))]
                    }],
                    query: "needle".into(),
                    limit: 10,
                    offset: 0,
                }
            ),
            Err(ContentError::Unavailable)
        );
    }

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
