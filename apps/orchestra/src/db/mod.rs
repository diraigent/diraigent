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
    let conn = Connection::open(&db_path)?;

    // Apply schema (all statements are IF NOT EXISTS, safe to re-run)
    conn.execute_batch(include_str!("schema.sql"))?;

    tracing::info!("local db: {}", db_path.display());
    Ok(Arc::new(Mutex::new(conn)))
}

/// Generate a unique ID (for rows that don't come from the API).
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}
