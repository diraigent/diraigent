//! Local SQLite persistence for orchestra operational state.
//!
//! Used when `orchestration_mode = local`. The orchestra owns task lifecycle
//! state here and syncs summaries to the API for display.

pub mod task_execution;
pub mod task_logs;
pub mod task_updates;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Thread-safe wrapper around a SQLite connection.
///
/// SQLite in WAL mode allows concurrent reads, but writes are serialized.
/// We use a Mutex to ensure safe access from async tokio tasks.
pub type Db = Arc<Mutex<Connection>>;

/// Open (or create) the orchestra SQLite database and run the embedded schema.
pub fn open(data_dir: &Path) -> Result<Db> {
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join("orchestra.db");
    let mut conn = Connection::open(&db_path)?;

    // Apply schema (all statements are IF NOT EXISTS, safe to re-run)
    conn.execute_batch(include_str!("schema.sql"))?;

    if conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))? < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(include_str!("migrations/001_retire_playbooks.sql"))?;
        tx.commit()?;
    }

    tracing::info!("local db: {}", db_path.display());
    Ok(Arc::new(Mutex::new(conn)))
}

/// Generate a unique ID (for rows that don't come from the API).
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrade_preserves_work_and_holds_legacy_stages() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("orchestra.db")).unwrap();
        conn.execute_batch(include_str!("schema.sql")).unwrap();
        conn.execute_batch("INSERT INTO task_execution (task_id, project_id, state, playbook_step, assigned_agent_id)
            VALUES ('review', 'p', 'wait:review', 1, 'agent'),
                   ('queued-review', 'p', 'ready', 1, 'agent'),
                   ('active', 'p', 'implement', 0, 'agent'),
                   ('direct', 'p', 'ready', 0, NULL),
                   ('done', 'p', 'done', 3, 'agent');").unwrap();
        drop(conn);
        let db = open(dir.path()).unwrap();
        for id in ["review", "queued-review", "active"] {
            let task = task_execution::get(&db, id).unwrap().unwrap();
            assert_eq!(task["state"], "human_review");
            assert!(task["assigned_agent_id"].is_null());
            assert!(task.get("playbook_step").is_none());
        }
        assert_eq!(
            task_execution::get(&db, "done").unwrap().unwrap()["state"],
            "done"
        );
        assert_eq!(
            task_execution::get(&db, "direct").unwrap().unwrap()["state"],
            "ready"
        );
        // Claim is exclusive, completion is terminal, and sync row indices align.
        task_execution::claim(&db, "direct", "agent").unwrap();
        assert!(task_execution::claim(&db, "direct", "other-agent").is_err());
        assert!(task_execution::transition(&db, "direct", "review").is_err());
        task_execution::transition(&db, "direct", "done").unwrap();
        let summaries = task_execution::get_unsynced(&db).unwrap();
        let summary = summaries.iter().find(|s| s["task_id"] == "direct").unwrap();
        assert_eq!(summary["assigned_agent_id"], "agent");
        assert!(!summary["completed_at"].is_null());
        drop(db);
        let reopened = open(dir.path()).unwrap();
        assert_eq!(
            task_execution::get(&reopened, "direct").unwrap().unwrap()["state"],
            "done"
        );
    }
}
