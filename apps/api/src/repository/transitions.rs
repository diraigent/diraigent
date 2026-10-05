use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::tasks::{check_dependencies_met, get_task_by_id};

// ── State Transitions ──

pub async fn transition_task(
    pool: &PgPool,
    task_id: Uuid,
    target_state: &str,
) -> Result<Task, AppError> {
    let existing = get_task_by_id(pool, task_id).await?;
    if !can_transition(&existing.state, target_state) {
        return Err(AppError::UnprocessableEntity(format!(
            "Cannot transition from '{}' to '{}'",
            existing.state, target_state
        )));
    }
    if target_state == "ready" {
        check_dependencies_met(pool, task_id).await?;
    }
    let completed_at = if target_state == "done" {
        Some(Utc::now())
    } else {
        None
    };
    let clear_agent = matches!(
        target_state,
        "ready" | "backlog" | "human_review" | "cancelled"
    );
    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task SET state = $2, completed_at = $3, state_entered_at = now(),
         assigned_agent_id = CASE WHEN $4 THEN NULL ELSE assigned_agent_id END,
         claimed_at = CASE WHEN $4 THEN NULL ELSE claimed_at END WHERE id = $1 AND state = $5 RETURNING *"
    ).bind(task_id).bind(target_state).bind(completed_at).bind(clear_agent).bind(&existing.state).fetch_optional(pool).await?
        .ok_or_else(|| AppError::Conflict("Task state changed concurrently; reload before retrying".into()))?;
    Ok(task)
}

pub async fn claim_task(pool: &PgPool, task_id: Uuid, agent_id: Uuid) -> Result<Task, AppError> {
    check_dependencies_met(pool, task_id).await?;
    sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task SET state = 'working', assigned_agent_id = $2, claimed_at = now(), state_entered_at = now()
         WHERE id = $1 AND state = 'ready' RETURNING *"
    ).bind(task_id).bind(agent_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::UnprocessableEntity("Task is not ready or was claimed by another agent".into()))
}

pub async fn resolve_task_mode(_pool: &PgPool, task: &Task) -> Result<String, AppError> {
    Ok(task.context["mode"]
        .as_str()
        .unwrap_or("working")
        .to_owned())
}

pub async fn release_task(pool: &PgPool, task_id: Uuid) -> Result<Task, AppError> {
    let existing = get_task_by_id(pool, task_id).await?;

    // Can only release from an active step (non-lifecycle state)
    if existing.state != "working" {
        return Err(AppError::UnprocessableEntity(
            "Task must be in an active step to release".into(),
        ));
    }

    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET state = 'ready', assigned_agent_id = NULL, claimed_at = NULL, state_entered_at = now()
         WHERE id = $1 RETURNING *",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await?;

    Ok(task)
}
