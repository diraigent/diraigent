//! `POST /orchestra/sync` — receive batched state updates from the orchestra.
//!
//! This is the write path for orchestra-managed tasks. The orchestra pushes
//! task state summaries, progress updates, and changed files here.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;

pub fn routes() -> Router<AppState> {
    Router::new().route("/orchestra/sync", post(receive_sync))
}

/// Batch payload from the orchestra.
#[derive(serde::Deserialize)]
struct SyncBatch {
    #[serde(default)]
    task_states: Vec<TaskStatePush>,
    #[serde(default)]
    task_updates: Vec<TaskUpdatePush>,
    #[serde(default)]
    changed_files: Vec<ChangedFilePush>,
}

#[derive(serde::Deserialize)]
struct TaskStatePush {
    task_id: String,
    state: String,
    playbook_step: Option<i32>,
    assigned_agent_id: Option<String>,
    claimed_at: Option<String>,
    completed_at: Option<String>,
    state_entered_at: Option<String>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cost_usd: Option<f64>,
}

#[derive(serde::Deserialize)]
struct TaskUpdatePush {
    id: String,
    task_id: String,
    agent_id: Option<String>,
    kind: String,
    content: String,
    created_at: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChangedFilePush {
    task_id: String,
    path: String,
    change_type: String,
}

async fn receive_sync(
    State(state): State<AppState>,
    AuthUser(_user_id): AuthUser,
    Json(batch): Json<SyncBatch>,
) -> Result<Json<serde_json::Value>, AppError> {
    let pool = &state.pool;
    let mut synced_tasks = 0u32;
    let mut synced_updates = 0u32;
    let mut synced_files = 0u32;

    // 1. Upsert task states (only for orchestra-managed tasks)
    for ts in &batch.task_states {
        let task_id: uuid::Uuid = ts
            .task_id
            .parse()
            .map_err(|_| AppError::Validation(format!("invalid task_id: {}", ts.task_id)))?;

        let rows = sqlx::query(
            r#"UPDATE diraigent.task
               SET state = $1,
                   playbook_step = COALESCE($2, playbook_step),
                   assigned_agent_id = $3::uuid,
                   claimed_at = $4::timestamptz,
                   completed_at = $5::timestamptz,
                   state_entered_at = COALESCE($6::timestamptz, now()),
                   input_tokens = COALESCE($7, input_tokens),
                   output_tokens = COALESCE($8, output_tokens),
                   cost_usd = COALESCE($9, cost_usd),
                   updated_at = now()
               WHERE id = $10 AND state_managed_by = 'orchestra'"#,
        )
        .bind(&ts.state)
        .bind(ts.playbook_step)
        .bind(&ts.assigned_agent_id)
        .bind(&ts.claimed_at)
        .bind(&ts.completed_at)
        .bind(&ts.state_entered_at)
        .bind(ts.input_tokens)
        .bind(ts.output_tokens)
        .bind(ts.cost_usd)
        .bind(task_id)
        .execute(pool)
        .await?;

        if rows.rows_affected() > 0 {
            synced_tasks += 1;
        }
    }

    // 2. Insert task updates (idempotent via ON CONFLICT)
    for tu in &batch.task_updates {
        let update_id: uuid::Uuid = tu
            .id
            .parse()
            .map_err(|_| AppError::Validation(format!("invalid update id: {}", tu.id)))?;
        let task_id: uuid::Uuid = tu
            .task_id
            .parse()
            .map_err(|_| AppError::Validation(format!("invalid task_id: {}", tu.task_id)))?;
        let agent_id: Option<uuid::Uuid> = tu
            .agent_id
            .as_ref()
            .map(|s| s.parse())
            .transpose()
            .map_err(|_| AppError::Validation("invalid agent_id".into()))?;

        sqlx::query(
            r#"INSERT INTO diraigent.task_update (id, task_id, agent_id, kind, content, created_at)
               VALUES ($1, $2, $3, $4, $5, COALESCE($6::timestamptz, now()))
               ON CONFLICT (id) DO NOTHING"#,
        )
        .bind(update_id)
        .bind(task_id)
        .bind(agent_id)
        .bind(&tu.kind)
        .bind(&tu.content)
        .bind(&tu.created_at)
        .execute(pool)
        .await?;

        synced_updates += 1;
    }

    // 3. Insert changed files
    for cf in &batch.changed_files {
        let task_id: uuid::Uuid = cf
            .task_id
            .parse()
            .map_err(|_| AppError::Validation(format!("invalid task_id: {}", cf.task_id)))?;

        sqlx::query(
            r#"INSERT INTO diraigent.task_changed_file (task_id, path, change_type)
               VALUES ($1, $2, $3)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(task_id)
        .bind(&cf.path)
        .bind(&cf.change_type)
        .execute(pool)
        .await?;

        synced_files += 1;
    }

    Ok(Json(serde_json::json!({
        "synced_tasks": synced_tasks,
        "synced_updates": synced_updates,
        "synced_files": synced_files,
    })))
}
