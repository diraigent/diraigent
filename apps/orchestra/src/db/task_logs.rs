//! CRUD for the `task_log`, `file_lock`, `verification`, and `event` tables.

use super::Db;
use anyhow::Result;
use serde_json::Value;

// ── Task Logs ───────────────────────────────────────────────────────

/// Insert a task execution log.
pub fn insert_log(
    db: &Db,
    project_id: &str,
    task_id: &str,
    step_name: &str,
    content: &str,
) -> Result<String> {
    let id = super::new_id();
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO task_log (id, project_id, task_id, step_name, content)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, project_id, task_id, step_name, content],
    )?;
    Ok(id)
}

// ── File Locks ──────────────────────────────────────────────────────

/// Acquire file locks for a task. Returns Err if any path conflicts with an existing lock.
pub fn acquire_locks(
    db: &Db,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    paths: &[String],
) -> Result<Vec<String>> {
    let conn = db.lock().unwrap();
    let mut ids = Vec::new();

    for path in paths {
        // Check for conflicting locks (same project, overlapping path, different task)
        let conflict: Option<String> = conn
            .query_row(
                "SELECT task_id FROM file_lock
                 WHERE project_id = ?1 AND path_glob = ?2 AND task_id != ?3",
                rusqlite::params![project_id, path, task_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(holder) = conflict {
            anyhow::bail!("file lock conflict: {path} held by task {holder}");
        }

        let id = super::new_id();
        conn.execute(
            "INSERT OR IGNORE INTO file_lock (id, project_id, task_id, path_glob, locked_by)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, project_id, task_id, path, agent_id],
        )?;
        ids.push(id);
    }
    Ok(ids)
}

/// Release all file locks for a task.
pub fn release_locks(db: &Db, task_id: &str) -> Result<usize> {
    let conn = db.lock().unwrap();
    let rows = conn.execute(
        "DELETE FROM file_lock WHERE task_id = ?1",
        rusqlite::params![task_id],
    )?;
    Ok(rows)
}

// ── Verifications ───────────────────────────────────────────────────

/// Insert a verification result.
#[allow(clippy::too_many_arguments)]
pub fn insert_verification(
    db: &Db,
    project_id: &str,
    task_id: Option<&str>,
    agent_id: Option<&str>,
    kind: &str,
    status: &str,
    title: &str,
    detail: Option<&str>,
) -> Result<String> {
    let id = super::new_id();
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO verification (id, project_id, task_id, agent_id, kind, status, title, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id, project_id, task_id, agent_id, kind, status, title, detail
        ],
    )?;
    Ok(id)
}

/// Get verifications for a project.
pub fn list_verifications(db: &Db, project_id: &str) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, project_id, task_id, agent_id, kind, status, title, detail, created_at
         FROM verification WHERE project_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![project_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "task_id": row.get::<_, Option<String>>(2)?,
                "agent_id": row.get::<_, Option<String>>(3)?,
                "kind": row.get::<_, String>(4)?,
                "status": row.get::<_, String>(5)?,
                "title": row.get::<_, String>(6)?,
                "detail": row.get::<_, Option<String>>(7)?,
                "created_at": row.get::<_, String>(8)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── Events ──────────────────────────────────────────────────────────

/// Insert an event.
pub fn insert_event(
    db: &Db,
    project_id: &str,
    kind: &str,
    title: &str,
    severity: &str,
    related_task_id: Option<&str>,
    agent_id: Option<&str>,
) -> Result<String> {
    let id = super::new_id();
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT INTO event (id, project_id, kind, title, severity, related_task_id, agent_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            project_id,
            kind,
            title,
            severity,
            related_task_id,
            agent_id
        ],
    )?;
    Ok(id)
}

/// Get unsynced events.
pub fn get_unsynced_events(db: &Db) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, project_id, kind, title, severity, related_task_id, agent_id, created_at
         FROM event WHERE synced = 0 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "kind": row.get::<_, String>(2)?,
                "title": row.get::<_, String>(3)?,
                "severity": row.get::<_, String>(4)?,
                "related_task_id": row.get::<_, Option<String>>(5)?,
                "agent_id": row.get::<_, Option<String>>(6)?,
                "created_at": row.get::<_, String>(7)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Mark events as synced.
pub fn mark_events_synced(db: &Db, ids: &[String]) -> Result<()> {
    let conn = db.lock().unwrap();
    for id in ids {
        conn.execute(
            "UPDATE event SET synced = 1 WHERE id = ?1",
            rusqlite::params![id],
        )?;
    }
    Ok(())
}
