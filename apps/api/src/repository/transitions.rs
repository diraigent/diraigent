use chrono::Utc;
use diraigent_types::StepProfile;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::playbooks::get_playbook_by_id;
use super::tasks::{check_dependencies_met, get_task_by_id, validate_playbook_step};

// ── State Transitions ──

pub async fn transition_task(
    pool: &PgPool,
    task_id: Uuid,
    target_state: &str,
    playbook_step: Option<i32>,
) -> Result<Task, AppError> {
    if target_state.is_empty() {
        return Err(AppError::Validation("State cannot be empty".into()));
    }

    let existing = get_task_by_id(pool, task_id).await?;

    // Auto-pipeline: when a non-final step transitions to "done", redirect to
    // "wait:<next_step>" instead of terminal "done". Only fetch the playbook
    // when a redirect is actually possible (step state with a playbook).
    let mut step_validated_by_pipeline = false;
    let (effective_target, effective_step) = if target_state == "done"
        && !is_lifecycle_state(&existing.state)
        && let Some(playbook_id) = existing.playbook_id
    {
        let playbook = get_playbook_by_id(pool, playbook_id).await?;
        let current = existing.playbook_step.unwrap_or(0) as usize;
        let next = current + 1;
        let total = playbook.steps.as_array().map(|a| a.len()).unwrap_or(0);

        if next < total {
            let next_name = playbook
                .steps
                .as_array()
                .and_then(|a| a.get(next))
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("implement");
            // Pipeline already verified next < total, so skip validate_playbook_step.
            step_validated_by_pipeline = true;
            (format!("wait:{next_name}"), Some(next as i32))
        } else {
            // Final step — "done" passes through unchanged.
            (target_state.to_string(), playbook_step)
        }
    } else {
        (target_state.to_string(), playbook_step)
    };
    let target_state = &effective_target;
    let playbook_step = effective_step;

    if !can_transition(&existing.state, target_state) {
        return Err(AppError::UnprocessableEntity(format!(
            "Cannot transition from '{}' to '{}'",
            existing.state, target_state
        )));
    }

    // Validate caller-supplied playbook_step. Skip when pipeline advancement
    // already computed and validated the step (avoids a redundant playbook fetch).
    if let Some(step) = playbook_step
        && !step_validated_by_pipeline
    {
        validate_playbook_step(pool, step, existing.playbook_id).await?;
    }

    // Enforce dependency blocking: cannot transition to ready/wait:* if blockers are not done.
    // Exception: when releasing from a step state (e.g. implement → ready), skip the check
    // so agents can release blocked tasks back to the queue.
    if (target_state == "ready" || is_wait_state(target_state))
        && is_lifecycle_state(&existing.state)
    {
        check_dependencies_met(pool, task_id).await?;
    }

    // ── Step regression ──
    // When a non-implement step (review, dream, etc.) releases back to ready,
    // regress playbook_step to the previous implement step so the implement
    // agent can apply the feedback.
    if target_state == "ready"
        && !is_lifecycle_state(&existing.state)
        && let Some(playbook_id) = existing.playbook_id
    {
        let playbook = get_playbook_by_id(pool, playbook_id).await?;
        let current_step = existing.playbook_step.unwrap_or(0) as usize;

        if let Some(steps) = playbook.steps.as_array() {
            let current_json = steps.get(current_step);
            let current_retriable = current_json.map(is_retriable_step).unwrap_or(true);

            if !current_retriable {
                // Find the previous retriable step to regress to
                for prev in (0..current_step).rev() {
                    if let Some(prev_step) = steps.get(prev)
                        && is_retriable_step(prev_step)
                    {
                        let task = sqlx::query_as::<_, Task>(
                            "UPDATE diraigent.task
                                 SET state = 'ready', playbook_step = $2,
                                     assigned_agent_id = NULL, claimed_at = NULL
                                 WHERE id = $1 RETURNING *",
                        )
                        .bind(task_id)
                        .bind(prev as i32)
                        .fetch_one(pool)
                        .await?;
                        return Ok(task);
                    }
                }
            }
        }
    }

    let completed_at = if target_state == "done" {
        Some(Utc::now())
    } else {
        None
    };

    // Clear agent when releasing (ready) or entering wait state between pipeline steps
    let clear_agent = (target_state == "ready" || is_wait_state(target_state))
        && !is_lifecycle_state(&existing.state);

    let task = match (clear_agent, playbook_step) {
        (true, Some(step)) => {
            sqlx::query_as::<_, Task>(
                "UPDATE diraigent.task
                 SET state = $2, assigned_agent_id = NULL, claimed_at = NULL, completed_at = $3, playbook_step = $4
                 WHERE id = $1 RETURNING *",
            )
            .bind(task_id)
            .bind(target_state)
            .bind(completed_at)
            .bind(step)
            .fetch_one(pool)
            .await?
        }
        (true, None) => {
            sqlx::query_as::<_, Task>(
                "UPDATE diraigent.task
                 SET state = $2, assigned_agent_id = NULL, claimed_at = NULL, completed_at = $3
                 WHERE id = $1 RETURNING *",
            )
            .bind(task_id)
            .bind(target_state)
            .bind(completed_at)
            .fetch_one(pool)
            .await?
        }
        (false, Some(step)) => {
            sqlx::query_as::<_, Task>(
                "UPDATE diraigent.task SET state = $2, completed_at = $3, playbook_step = $4
                 WHERE id = $1 RETURNING *",
            )
            .bind(task_id)
            .bind(target_state)
            .bind(completed_at)
            .bind(step)
            .fetch_one(pool)
            .await?
        }
        (false, None) => {
            sqlx::query_as::<_, Task>(
                "UPDATE diraigent.task SET state = $2, completed_at = $3
                 WHERE id = $1 RETURNING *",
            )
            .bind(task_id)
            .bind(target_state)
            .bind(completed_at)
            .fetch_one(pool)
            .await?
        }
    };

    Ok(task)
}

/// Check if a playbook step is retriable (can be regressed to on rejection).
///
/// Reads `"retriable"` from the step JSON if present, otherwise falls back
/// to name-prefix classification (implement-like steps are retriable).
fn is_retriable_step(step: &serde_json::Value) -> bool {
    if let Some(v) = step.get("retriable").and_then(|v| v.as_bool()) {
        return v;
    }
    // Fallback: classify by name prefix (backward compat for steps without the field)
    let name = step["name"].as_str().unwrap_or("");
    StepProfile::for_step(name).is_implement()
}

pub async fn claim_task(pool: &PgPool, task_id: Uuid, agent_id: Uuid) -> Result<Task, AppError> {
    // Look up the task to determine the step name from its playbook
    let existing = get_task_by_id(pool, task_id).await?;

    // Claimable from "ready" or "wait:<step>"
    let step_name = if existing.state == "ready" {
        resolve_step_name(pool, &existing).await?
    } else if let Some(next) = crate::models::wait_target(&existing.state) {
        next.to_string()
    } else {
        return Err(AppError::UnprocessableEntity(
            "Task is not in a claimable state (ready or wait:*)".into(),
        ));
    };

    let current_state = &existing.state;

    // Atomic: only claim if state hasn't changed
    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET state = $3, assigned_agent_id = $2, claimed_at = now()
         WHERE id = $1 AND state = $4
         RETURNING *",
    )
    .bind(task_id)
    .bind(agent_id)
    .bind(&step_name)
    .bind(current_state)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::UnprocessableEntity("Task was claimed by another agent".into()))?;

    Ok(task)
}

/// Public wrapper for route-level authz to determine the step a claim will enter.
pub async fn resolve_claim_step_name(pool: &PgPool, task: &Task) -> Result<String, AppError> {
    // For wait:<step> states, the step name is embedded in the state.
    if let Some(next) = crate::models::wait_target(&task.state) {
        return Ok(next.to_string());
    }
    resolve_step_name(pool, task).await
}

/// Resolve the current playbook step name for a task.
/// Returns the step name from the playbook, or "working" for tasks without a playbook.
pub(crate) async fn resolve_step_name(pool: &PgPool, task: &Task) -> Result<String, AppError> {
    if let Some(playbook_id) = task.playbook_id {
        let playbook = get_playbook_by_id(pool, playbook_id).await?;
        let step_index = task.playbook_step.unwrap_or(0) as usize;
        let name = playbook
            .steps
            .as_array()
            .and_then(|steps| steps.get(step_index))
            .and_then(|step| step.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("implement");
        Ok(name.to_string())
    } else {
        Ok("working".to_string())
    }
}

pub async fn release_task(pool: &PgPool, task_id: Uuid) -> Result<Task, AppError> {
    let existing = get_task_by_id(pool, task_id).await?;

    // Can only release from an active step (non-lifecycle state)
    if is_lifecycle_state(&existing.state) {
        return Err(AppError::UnprocessableEntity(
            "Task must be in an active step to release".into(),
        ));
    }

    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET state = 'ready', assigned_agent_id = NULL, claimed_at = NULL
         WHERE id = $1 RETURNING *",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await?;

    Ok(task)
}
