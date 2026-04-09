use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::projects::get_project_by_id;
use super::transitions::resolve_step_name;
use super::{Table, delete_by_id, fetch_by_id};

const TASK_FILTERS_WHERE: &str = "WHERE project_id = $1 \
    AND ($2::text IS NULL OR state = $2) \
    AND ($3::text IS NULL OR kind = $3) \
    AND ($4::uuid IS NULL OR assigned_agent_id = $4) \
    AND ($5::text IS NULL OR title ILIKE $5) \
    AND ($6::timestamptz IS NULL OR state NOT IN ('done', 'cancelled') OR COALESCE(completed_at, updated_at) >= $6) \
    AND ($7::uuid IS NULL OR decision_id = $7) \
    AND ($8::uuid IS NULL OR parent_id = $8)";

// ── Tasks ──

pub async fn create_task(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateTask,
    created_by: Uuid,
) -> Result<Task, AppError> {
    // Verify project exists and get default playbook
    let project = get_project_by_id(pool, project_id).await?;

    let kind = req.kind.as_deref().unwrap_or("feature");

    let context = req
        .context
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));
    let capabilities = req.required_capabilities.clone().unwrap_or_default();
    let file_scope = req.file_scope.clone().unwrap_or_default();
    let urgent = req.urgent.unwrap_or(false);

    // Use explicit playbook_name, or fall back to project default
    let playbook_name = req
        .playbook_name
        .clone()
        .or_else(|| project.default_playbook_name.clone());

    // Tasks with a playbook start as "ready"; tasks without stay in "backlog".
    let initial_state = if playbook_name.is_some() { "ready" } else { "backlog" };

    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO diraigent.task (project_id, title, kind, state, urgent, context, required_capabilities, playbook_name, playbook_step, decision_id, created_by, file_scope, parent_id, state_entered_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, now())
         RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(kind)
    .bind(initial_state)
    .bind(urgent)
    .bind(&context)
    .bind(&capabilities)
    .bind(playbook_name.as_deref())
    .bind(if playbook_name.is_some() { Some(0i32) } else { None })
    .bind(req.decision_id)
    .bind(created_by)
    .bind(&file_scope)
    .bind(req.parent_id)
    .fetch_one(pool)
    .await?;

    Ok(task)
}

pub async fn get_task_by_id(pool: &PgPool, task_id: Uuid) -> Result<Task, AppError> {
    fetch_by_id(pool, Table::Task, task_id, "Task not found").await
}

/// Fetch multiple tasks by ID in a single query.
pub async fn get_tasks_by_ids(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<Task>, AppError> {
    let tasks = sqlx::query_as::<_, Task>("SELECT * FROM diraigent.task WHERE id = ANY($1)")
        .bind(ids)
        .fetch_all(pool)
        .await?;
    Ok(tasks)
}

pub async fn list_tasks_by_decision(
    pool: &PgPool,
    decision_id: Uuid,
) -> Result<Vec<TaskSummaryForDecision>, AppError> {
    let items = sqlx::query_as::<_, TaskSummaryForDecision>(
        "SELECT id, number, title, kind, state, urgent, created_at
         FROM diraigent.task
         WHERE decision_id = $1
         ORDER BY created_at ASC",
    )
    .bind(decision_id)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

pub async fn list_tasks(
    pool: &PgPool,
    project_id: Uuid,
    filters: &TaskFilters,
) -> Result<Vec<Task>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    // Use ready_only special query
    if filters.ready_only == Some(true) {
        return list_ready_tasks(pool, project_id, limit, offset).await;
    }

    let search_pattern = filters.search.as_deref().map(|s| format!("%{}%", s));

    let mut extra_where = String::new();
    if filters.work_id.is_some() {
        extra_where
            .push_str(" AND id IN (SELECT task_id FROM diraigent.task_work WHERE work_id = $9)");
    }
    if filters.unlinked == Some(true) {
        extra_where.push_str(
            " AND NOT EXISTS (SELECT 1 FROM diraigent.task_work tw WHERE tw.task_id = diraigent.task.id)",
        );
    }
    if filters.root_only == Some(true) {
        extra_where.push_str(" AND parent_id IS NULL");
    }

    let (limit_param, offset_param) = if filters.work_id.is_some() {
        ("$10", "$11")
    } else {
        ("$9", "$10")
    };

    let sql = format!(
        "SELECT * FROM diraigent.task {}{} ORDER BY created_at DESC LIMIT {} OFFSET {}",
        TASK_FILTERS_WHERE, extra_where, limit_param, offset_param
    );
    let mut query = sqlx::query_as::<_, Task>(&sql)
        .bind(project_id)
        .bind(&filters.state)
        .bind(&filters.kind)
        .bind(filters.agent_id)
        .bind(&search_pattern)
        .bind(filters.hide_done_before)
        .bind(filters.decision_id)
        .bind(filters.parent_id);

    if let Some(work_id) = filters.work_id {
        query = query.bind(work_id);
    }

    let tasks = query.bind(limit).bind(offset).fetch_all(pool).await?;

    Ok(tasks)
}

pub async fn list_ready_tasks(
    pool: &PgPool,
    project_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Task>, AppError> {
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT t.* FROM diraigent.task t
         WHERE t.project_id = $1
           AND (t.state = 'ready' OR t.state LIKE 'wait:%')
           AND NOT EXISTS (
               SELECT 1 FROM diraigent.task_dependency td
               JOIN diraigent.task t2 ON td.depends_on = t2.id
               WHERE td.task_id = t.id AND t2.state != 'done'
           )
         ORDER BY t.urgent DESC, t.created_at ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(tasks)
}

/// Validates that a playbook_step value is non-negative.
pub(crate) async fn validate_playbook_step(step: i32) -> Result<(), AppError> {
    if step < 0 {
        return Err(AppError::UnprocessableEntity(
            "playbook_step cannot be negative".into(),
        ));
    }
    Ok(())
}

pub async fn update_task(pool: &PgPool, task_id: Uuid, req: &UpdateTask) -> Result<Task, AppError> {
    let existing = get_task_by_id(pool, task_id).await?;

    let playbook_name = req
        .playbook_name
        .as_deref()
        .or(existing.playbook_name.as_deref());

    if let Some(step) = req.playbook_step {
        validate_playbook_step(step).await?;
    }

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let kind = req.kind.as_deref().unwrap_or(&existing.kind);
    let urgent = req.urgent.unwrap_or(existing.urgent);
    let context = req.context.as_ref().unwrap_or(&existing.context);
    let capabilities = req
        .required_capabilities
        .as_ref()
        .unwrap_or(&existing.required_capabilities);
    let playbook_step = req.playbook_step.or(existing.playbook_step);
    let flagged = req.flagged.unwrap_or(existing.flagged);
    let file_scope = req.file_scope.as_ref().unwrap_or(&existing.file_scope);

    // Double-Option: None → keep existing, Some(v) → use v (which may be None to clear).
    let parent_id = match &req.parent_id {
        Some(v) => *v,
        None => existing.parent_id,
    };

    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET title = $2, kind = $3, urgent = $4, context = $5, required_capabilities = $6, playbook_step = $7, playbook_name = $8, flagged = $9, file_scope = $10, parent_id = $11
         WHERE id = $1 RETURNING *",
    )
    .bind(task_id)
    .bind(title)
    .bind(kind)
    .bind(urgent)
    .bind(context)
    .bind(capabilities)
    .bind(playbook_step)
    .bind(playbook_name)
    .bind(flagged)
    .bind(file_scope)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;

    Ok(task)
}

pub async fn delete_task(pool: &PgPool, task_id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Task, task_id, "Task not found").await
}

// ── Dependencies ──

/// Check that all dependencies of a task are in 'done' state.
/// Returns Ok(()) if no dependencies or all are done.
/// Returns UnprocessableEntity with blocking task details otherwise.
pub(crate) async fn check_dependencies_met(pool: &PgPool, task_id: Uuid) -> Result<(), AppError> {
    let blockers: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT t.id, t.title, t.state
         FROM diraigent.task_dependency td
         JOIN diraigent.task t ON td.depends_on = t.id
         WHERE td.task_id = $1 AND t.state != 'done'",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    if !blockers.is_empty() {
        let details: Vec<String> = blockers
            .iter()
            .map(|(id, title, state)| format!("{} ('{}', state={})", id, title, state))
            .collect();
        return Err(AppError::UnprocessableEntity(format!(
            "Cannot transition to ready: blocked by {} incomplete dependencies: {}",
            blockers.len(),
            details.join(", ")
        )));
    }

    Ok(())
}

/// Returns task IDs that have at least one unmet dependency (blocker not in 'done' state).
pub async fn list_blocked_task_ids(pool: &PgPool, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let ids: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT td.task_id
         FROM diraigent.task_dependency td
         JOIN diraigent.task t ON td.task_id = t.id
         JOIN diraigent.task blocker ON td.depends_on = blocker.id
         WHERE t.project_id = $1 AND blocker.state != 'done'",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(ids.into_iter().map(|(id,)| id).collect())
}

/// List direct children of a task (tasks whose parent_id matches).
pub async fn list_task_children(pool: &PgPool, parent_id: Uuid) -> Result<Vec<Task>, AppError> {
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT * FROM diraigent.task WHERE parent_id = $1 ORDER BY urgent DESC, created_at ASC",
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

/// Returns task IDs that have been flagged (bookmarked) by a user.
pub async fn list_flagged_task_ids(pool: &PgPool, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let ids: Vec<(Uuid,)> =
        sqlx::query_as("SELECT id FROM diraigent.task WHERE project_id = $1 AND flagged = true")
            .bind(project_id)
            .fetch_all(pool)
            .await?;
    Ok(ids.into_iter().map(|(id,)| id).collect())
}

pub async fn list_work_linked_task_ids(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT tw.task_id
         FROM diraigent.task_work tw
         JOIN diraigent.task t ON tw.task_id = t.id
         WHERE t.project_id = $1",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(ids.into_iter().map(|(id,)| id).collect())
}

/// Returns tasks that have at least one `kind='blocker'` update and are not done/cancelled.
pub async fn list_tasks_with_blocker_updates(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Task>, AppError> {
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT DISTINCT ON (t.id) t.*
         FROM diraigent.task t
         JOIN diraigent.task_update tu ON tu.task_id = t.id
         WHERE t.project_id = $1
           AND tu.kind = 'blocker'
           AND t.state NOT IN ('done', 'cancelled')
         ORDER BY t.id, t.updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

pub async fn add_dependency(
    pool: &PgPool,
    task_id: Uuid,
    depends_on: Uuid,
) -> Result<TaskDependency, AppError> {
    let dep = sqlx::query_as::<_, TaskDependency>(
        "INSERT INTO diraigent.task_dependency (task_id, depends_on)
         VALUES ($1, $2) RETURNING *",
    )
    .bind(task_id)
    .bind(depends_on)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Dependency already exists or invalid reference".into())
        }
        _ => e.into(),
    })?;

    Ok(dep)
}

pub async fn remove_dependency(
    pool: &PgPool,
    task_id: Uuid,
    depends_on: Uuid,
) -> Result<(), AppError> {
    let result =
        sqlx::query("DELETE FROM diraigent.task_dependency WHERE task_id = $1 AND depends_on = $2")
            .bind(task_id)
            .bind(depends_on)
            .execute(pool)
            .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Dependency not found".into()));
    }
    Ok(())
}

/// List dependencies of a task with enriched info (title, state).
pub async fn list_dependencies(pool: &PgPool, task_id: Uuid) -> Result<TaskDependencies, AppError> {
    let depends_on: Vec<TaskDependencyInfo> = sqlx::query_as(
        "SELECT td.task_id, td.depends_on, t.title, t.state
         FROM diraigent.task_dependency td
         JOIN diraigent.task t ON td.depends_on = t.id
         WHERE td.task_id = $1
         ORDER BY t.created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    let blocks: Vec<TaskDependencyInfo> = sqlx::query_as(
        "SELECT td.task_id, td.depends_on, t.title, t.state
         FROM diraigent.task_dependency td
         JOIN diraigent.task t ON td.task_id = t.id
         WHERE td.depends_on = $1
         ORDER BY t.created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(TaskDependencies { depends_on, blocks })
}

// ── Task Cost Metrics ──

/// Accumulate LLM token usage and cost on a task.
/// Values are added to the existing totals so costs across multiple steps
/// are aggregated automatically.
pub async fn update_task_cost(
    pool: &PgPool,
    task_id: Uuid,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
) -> Result<Task, AppError> {
    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET input_tokens  = input_tokens  + $2,
             output_tokens = output_tokens + $3,
             cost_usd      = cost_usd      + $4,
             updated_at    = NOW()
         WHERE id = $1
         RETURNING *",
    )
    .bind(task_id)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(cost_usd)
    .fetch_one(pool)
    .await?;

    Ok(task)
}

// ── Task Updates ──

pub async fn create_task_update(
    pool: &PgPool,
    task_id: Uuid,
    req: &CreateTaskUpdate,
    user_id: Option<Uuid>,
) -> Result<TaskUpdate, AppError> {
    let kind = req.kind.as_deref().unwrap_or("progress");
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let update = sqlx::query_as::<_, TaskUpdate>(
        "INSERT INTO diraigent.task_update (task_id, agent_id, user_id, kind, content, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(task_id)
    .bind(req.agent_id)
    .bind(user_id)
    .bind(kind)
    .bind(&req.content)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(update)
}

pub async fn list_task_updates(
    pool: &PgPool,
    task_id: Uuid,
    p: &Pagination,
) -> Result<Vec<TaskUpdate>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let updates = sqlx::query_as::<_, TaskUpdate>(
        "SELECT * FROM diraigent.task_update WHERE task_id = $1
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(updates)
}

// ── Task Comments ──

pub async fn create_task_comment(
    pool: &PgPool,
    task_id: Uuid,
    req: &CreateTaskComment,
    user_id: Option<Uuid>,
) -> Result<TaskComment, AppError> {
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let comment = sqlx::query_as::<_, TaskComment>(
        "INSERT INTO diraigent.task_comment (task_id, agent_id, user_id, content, metadata)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(task_id)
    .bind(req.agent_id)
    .bind(user_id)
    .bind(&req.content)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(comment)
}

pub async fn list_task_comments(
    pool: &PgPool,
    task_id: Uuid,
    p: &Pagination,
) -> Result<Vec<TaskComment>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let comments = sqlx::query_as::<_, TaskComment>(
        "SELECT * FROM diraigent.task_comment WHERE task_id = $1
         ORDER BY created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(comments)
}

// ── Task Delegation ──

pub async fn delegate_task(
    pool: &PgPool,
    task_id: Uuid,
    delegated_by_agent_id: Uuid,
    to_agent_id: Uuid,
    role_id: Option<Uuid>,
) -> Result<Task, AppError> {
    // Look up task to resolve step name if needed
    let existing = get_task_by_id(pool, task_id).await?;
    let step_name = resolve_step_name(pool, &existing).await?;

    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET assigned_agent_id = $2, assigned_role_id = $3, delegated_by = $4, delegated_at = now(),
             state = CASE WHEN state IN ('ready', 'backlog') THEN $5 ELSE state END,
             claimed_at = CASE WHEN state IN ('ready', 'backlog') THEN now() ELSE claimed_at END,
             state_entered_at = CASE WHEN state IN ('ready', 'backlog') THEN now() ELSE state_entered_at END
         WHERE id = $1 RETURNING *",
    )
    .bind(task_id)
    .bind(to_agent_id)
    .bind(role_id)
    .bind(delegated_by_agent_id)
    .bind(&step_name)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".into()))?;

    Ok(task)
}

// ── Count ──

pub async fn count_tasks(
    pool: &PgPool,
    project_id: Uuid,
    filters: &TaskFilters,
) -> Result<i64, AppError> {
    if filters.ready_only == Some(true) {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM diraigent.task t
             WHERE t.project_id = $1
               AND t.state = 'ready'
               AND NOT EXISTS (
                   SELECT 1 FROM diraigent.task_dependency td
                   JOIN diraigent.task t2 ON td.depends_on = t2.id
                   WHERE td.task_id = t.id AND t2.state != 'done'
               )",
        )
        .bind(project_id)
        .fetch_one(pool)
        .await?;
        return Ok(row.0);
    }

    let search_pattern = filters.search.as_deref().map(|s| format!("%{}%", s));

    let mut extra_where = String::new();
    if filters.work_id.is_some() {
        extra_where
            .push_str(" AND id IN (SELECT task_id FROM diraigent.task_work WHERE work_id = $9)");
    }
    if filters.unlinked == Some(true) {
        extra_where.push_str(
            " AND NOT EXISTS (SELECT 1 FROM diraigent.task_work tw WHERE tw.task_id = diraigent.task.id)",
        );
    }
    if filters.root_only == Some(true) {
        extra_where.push_str(" AND parent_id IS NULL");
    }

    let sql = format!(
        "SELECT COUNT(*) FROM diraigent.task {}{}",
        TASK_FILTERS_WHERE, extra_where
    );
    let mut query = sqlx::query_as::<_, (i64,)>(&sql)
        .bind(project_id)
        .bind(&filters.state)
        .bind(&filters.kind)
        .bind(filters.agent_id)
        .bind(&search_pattern)
        .bind(filters.hide_done_before)
        .bind(filters.decision_id)
        .bind(filters.parent_id);

    if let Some(work_id) = filters.work_id {
        query = query.bind(work_id);
    }

    let row: (i64,) = query.fetch_one(pool).await?;

    Ok(row.0)
}

// ── Subtasks (parent-child) ──

pub async fn list_subtasks(
    pool: &PgPool,
    parent_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Task>, AppError> {
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT * FROM diraigent.task WHERE parent_id = $1
         ORDER BY created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(parent_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_playbook_step tests ──

    #[tokio::test]
    async fn validate_playbook_step_negative_step_is_rejected() {
        let result = validate_playbook_step(-1).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cannot be negative"),
            "expected 'cannot be negative', got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn validate_playbook_step_non_negative_is_ok() {
        assert!(validate_playbook_step(0).await.is_ok());
        assert!(validate_playbook_step(5).await.is_ok());
        assert!(validate_playbook_step(100).await.is_ok());
    }
}
