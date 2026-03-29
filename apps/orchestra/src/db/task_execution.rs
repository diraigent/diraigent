//! CRUD for the `task_execution` table — orchestra-owned task state.

use super::Db;
use anyhow::Result;
use serde_json::Value;

/// Insert a new task execution record (called when orchestra first learns about a task).
pub fn insert(db: &Db, task_id: &str, project_id: &str, state: &str) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO task_execution (task_id, project_id, state, state_entered_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        rusqlite::params![task_id, project_id, state],
    )?;
    Ok(())
}

/// Claim a task: transition from ready to a step name.
pub fn claim(db: &Db, task_id: &str, step_name: &str, agent_id: &str) -> Result<()> {
    let conn = db.lock().unwrap();
    let rows = conn.execute(
        "UPDATE task_execution
         SET state = ?1, assigned_agent_id = ?2,
             claimed_at = datetime('now'), state_entered_at = datetime('now'),
             last_synced_at = NULL
         WHERE task_id = ?3 AND state = 'ready'",
        rusqlite::params![step_name, agent_id, task_id],
    )?;
    if rows == 0 {
        anyhow::bail!("task {task_id} is not in 'ready' state");
    }
    Ok(())
}

/// Transition a task to a new state.
pub fn transition(db: &Db, task_id: &str, new_state: &str) -> Result<String> {
    let conn = db.lock().unwrap();
    let current: String = conn.query_row(
        "SELECT state FROM task_execution WHERE task_id = ?1",
        rusqlite::params![task_id],
        |row| row.get(0),
    )?;

    if !diraigent_types::state_machine::can_transition(&current, new_state) {
        anyhow::bail!("invalid transition: {current} → {new_state}");
    }

    let completed_at = if new_state == "done" || new_state == "cancelled" {
        "datetime('now')"
    } else {
        "NULL"
    };

    conn.execute(
        &format!(
            "UPDATE task_execution
             SET state = ?1, state_entered_at = datetime('now'),
                 completed_at = {completed_at}, last_synced_at = NULL
             WHERE task_id = ?2"
        ),
        rusqlite::params![new_state, task_id],
    )?;

    Ok(current)
}

/// Advance a task's playbook_step and set state to ready (pipeline advancement).
pub fn advance_step(db: &Db, task_id: &str) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE task_execution
         SET playbook_step = playbook_step + 1,
             state = 'ready',
             state_entered_at = datetime('now'),
             assigned_agent_id = NULL,
             claimed_at = NULL,
             last_synced_at = NULL
         WHERE task_id = ?1",
        rusqlite::params![task_id],
    )?;
    Ok(())
}

/// Regress a task's playbook_step to a specific index (step rejection).
pub fn regress_step(db: &Db, task_id: &str, step_index: i32) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE task_execution
         SET playbook_step = ?1,
             state = 'ready',
             state_entered_at = datetime('now'),
             assigned_agent_id = NULL,
             claimed_at = NULL,
             last_synced_at = NULL
         WHERE task_id = ?2",
        rusqlite::params![step_index, task_id],
    )?;
    Ok(())
}

/// Accumulate cost and tokens for a task.
pub fn add_cost(
    db: &Db,
    task_id: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
) -> Result<()> {
    let conn = db.lock().unwrap();
    conn.execute(
        "UPDATE task_execution
         SET input_tokens = input_tokens + ?1,
             output_tokens = output_tokens + ?2,
             cost_usd = cost_usd + ?3,
             last_synced_at = NULL
         WHERE task_id = ?4",
        rusqlite::params![input_tokens, output_tokens, cost_usd, task_id],
    )?;
    Ok(())
}

/// Get a task execution record as JSON (for compatibility with TaskSource trait).
pub fn get(db: &Db, task_id: &str) -> Result<Option<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT task_id, project_id, state, playbook_id, playbook_step,
                assigned_agent_id, claimed_at, completed_at, state_entered_at,
                input_tokens, output_tokens, cost_usd, created_at
         FROM task_execution WHERE task_id = ?1",
    )?;

    let row = stmt.query_row(rusqlite::params![task_id], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "project_id": row.get::<_, String>(1)?,
            "state": row.get::<_, String>(2)?,
            "playbook_id": row.get::<_, Option<String>>(3)?,
            "playbook_step": row.get::<_, Option<i32>>(4)?,
            "assigned_agent_id": row.get::<_, Option<String>>(5)?,
            "claimed_at": row.get::<_, Option<String>>(6)?,
            "completed_at": row.get::<_, Option<String>>(7)?,
            "state_entered_at": row.get::<_, Option<String>>(8)?,
            "input_tokens": row.get::<_, i64>(9)?,
            "output_tokens": row.get::<_, i64>(10)?,
            "cost_usd": row.get::<_, f64>(11)?,
            "created_at": row.get::<_, String>(12)?,
        }))
    });

    match row {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Get all tasks in ready state for a given project.
pub fn get_ready(db: &Db, project_id: &str) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT task_id FROM task_execution
         WHERE project_id = ?1 AND state = 'ready'
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![project_id], |row| {
            let id: String = row.get(0)?;
            Ok(serde_json::json!({"id": id}))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all unsynced task executions for the sync loop.
pub fn get_unsynced(db: &Db) -> Result<Vec<Value>> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT task_id, project_id, state, playbook_step,
                assigned_agent_id, claimed_at, completed_at, state_entered_at,
                input_tokens, output_tokens, cost_usd
         FROM task_execution
         WHERE last_synced_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "task_id": row.get::<_, String>(0)?,
                "state": row.get::<_, String>(1)?,
                "playbook_step": row.get::<_, Option<i32>>(2)?,
                "assigned_agent_id": row.get::<_, Option<String>>(3)?,
                "claimed_at": row.get::<_, Option<String>>(4)?,
                "completed_at": row.get::<_, Option<String>>(5)?,
                "state_entered_at": row.get::<_, Option<String>>(6)?,
                "input_tokens": row.get::<_, i64>(7)?,
                "output_tokens": row.get::<_, i64>(8)?,
                "cost_usd": row.get::<_, f64>(9)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Mark tasks as synced.
pub fn mark_synced(db: &Db, task_ids: &[String]) -> Result<()> {
    let conn = db.lock().unwrap();
    for id in task_ids {
        conn.execute(
            "UPDATE task_execution SET last_synced_at = datetime('now') WHERE task_id = ?1",
            rusqlite::params![id],
        )?;
    }
    Ok(())
}
