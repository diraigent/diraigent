//! CRUD for the `task_update` and `task_changed_file` tables.

use super::Db;
use anyhow::Result;
use serde_json::Value;

/// Insert a task update (progress, blocker, artifact, etc.).
pub fn insert(
    db: &Db,
    task_id: &str,
    agent_id: Option<&str>,
    kind: &str,
    content: &str,
) -> Result<String> {
    let id = super::new_id();
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO task_update (id, task_id, agent_id, kind, content)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, task_id, agent_id, kind, content],
    )?;
    Ok(id)
}

/// Get updates for a task.
pub fn list_for_task(db: &Db, task_id: &str) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, agent_id, kind, content, created_at
         FROM task_update WHERE task_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![task_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "task_id": row.get::<_, String>(1)?,
                "agent_id": row.get::<_, Option<String>>(2)?,
                "kind": row.get::<_, String>(3)?,
                "content": row.get::<_, String>(4)?,
                "created_at": row.get::<_, String>(5)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all unsynced updates.
pub fn get_unsynced(db: &Db) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, agent_id, kind, content, created_at
         FROM task_update WHERE synced = 0 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "task_id": row.get::<_, String>(1)?,
                "agent_id": row.get::<_, Option<String>>(2)?,
                "kind": row.get::<_, String>(3)?,
                "content": row.get::<_, String>(4)?,
                "created_at": row.get::<_, String>(5)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Mark updates as synced.
pub fn mark_synced(db: &Db, ids: &[String]) -> Result<()> {
    let conn = db.lock().unwrap();
    for id in ids {
        conn.execute(
            "UPDATE task_update SET synced = 1 WHERE id = ?1",
            rusqlite::params![id],
        )?;
    }
    Ok(())
}

/// Record a changed file.
pub fn insert_changed_file(
    db: &Db,
    task_id: &str,
    path: &str,
    change_type: &str,
) -> Result<String> {
    let id = super::new_id();
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO task_changed_file (id, task_id, path, change_type)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, task_id, path, change_type],
    )?;
    Ok(id)
}

/// Get unsynced changed files.
pub fn get_unsynced_changed_files(db: &Db) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, task_id, path, change_type
         FROM task_changed_file WHERE synced = 0",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "task_id": row.get::<_, String>(1)?,
                "path": row.get::<_, String>(2)?,
                "change_type": row.get::<_, String>(3)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Mark changed files as synced.
pub fn mark_changed_files_synced(db: &Db, ids: &[String]) -> Result<()> {
    let conn = db.lock().unwrap();
    for id in ids {
        conn.execute(
            "UPDATE task_changed_file SET synced = 1 WHERE id = ?1",
            rusqlite::params![id],
        )?;
    }
    Ok(())
}
