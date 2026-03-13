use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::playbooks::get_playbook_by_id;
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
    let priority = req.priority.unwrap_or(0);

    // Use explicit playbook_id, or fall back to project default
    let playbook_id = req.playbook_id.or(project.default_playbook_id);

    // Determine initial state: respect the playbook's initial_state field, fall back to
    // "ready" for backward compatibility. Tasks without a playbook always start in "backlog".
    let initial_state = if let Some(pb_id) = playbook_id {
        let playbook = get_playbook_by_id(pool, pb_id).await?;
        playbook.initial_state.clone()
    } else {
        "backlog".to_string()
    };

    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO diraigent.task (project_id, title, kind, state, priority, context, required_capabilities, playbook_id, playbook_step, decision_id, created_by, file_scope, parent_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(kind)
    .bind(initial_state)
    .bind(priority)
    .bind(&context)
    .bind(&capabilities)
    .bind(playbook_id)
    .bind(if playbook_id.is_some() { Some(0i32) } else { None })
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

pub async fn list_tasks_by_decision(
    pool: &PgPool,
    decision_id: Uuid,
) -> Result<Vec<TaskSummaryForDecision>, AppError> {
    let items = sqlx::query_as::<_, TaskSummaryForDecision>(
        "SELECT id, number, title, kind, state, priority, created_at
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
    if filters.goal_id.is_some() {
        extra_where
            .push_str(" AND id IN (SELECT task_id FROM diraigent.task_goal WHERE goal_id = $9)");
    }
    if filters.unlinked == Some(true) {
        extra_where.push_str(
            " AND NOT EXISTS (SELECT 1 FROM diraigent.task_goal tg WHERE tg.task_id = diraigent.task.id)",
        );
    }
    if filters.root_only == Some(true) {
        extra_where.push_str(" AND parent_id IS NULL");
    }

    let (limit_param, offset_param) = if filters.goal_id.is_some() {
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

    if let Some(goal_id) = filters.goal_id {
        query = query.bind(goal_id);
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
         ORDER BY t.priority ASC, t.created_at ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(project_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(tasks)
}

/// Validates that a playbook_step value is non-negative and within bounds of the assigned playbook.
/// If the task has no playbook_id, only the negative check applies.
pub(crate) async fn validate_playbook_step(
    pool: &PgPool,
    step: i32,
    playbook_id: Option<Uuid>,
) -> Result<(), AppError> {
    if step < 0 {
        return Err(AppError::UnprocessableEntity(
            "playbook_step cannot be negative".into(),
        ));
    }
    if let Some(playbook_id) = playbook_id {
        let playbook = get_playbook_by_id(pool, playbook_id).await?;
        let step_count = playbook.steps.as_array().map(|a| a.len()).unwrap_or(0);
        if step as usize >= step_count {
            return Err(AppError::UnprocessableEntity(format!(
                "playbook_step {} is out of range (playbook has {} steps)",
                step, step_count
            )));
        }
    }
    Ok(())
}

pub async fn update_task(pool: &PgPool, task_id: Uuid, req: &UpdateTask) -> Result<Task, AppError> {
    let existing = get_task_by_id(pool, task_id).await?;

    // Double-Option: None → keep existing, Some(v) → use v (which may be None to clear).
    let playbook_id = match &req.playbook_id {
        Some(v) => *v,
        None => existing.playbook_id,
    };

    if let Some(step) = req.playbook_step {
        validate_playbook_step(pool, step, playbook_id).await?;
    }

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let kind = req.kind.as_deref().unwrap_or(&existing.kind);
    let priority = req.priority.unwrap_or(existing.priority);
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
         SET title = $2, kind = $3, priority = $4, context = $5, required_capabilities = $6, playbook_step = $7, playbook_id = $8, flagged = $9, file_scope = $10, parent_id = $11
         WHERE id = $1 RETURNING *",
    )
    .bind(task_id)
    .bind(title)
    .bind(kind)
    .bind(priority)
    .bind(context)
    .bind(capabilities)
    .bind(playbook_step)
    .bind(playbook_id)
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
        "SELECT * FROM diraigent.task WHERE parent_id = $1 ORDER BY priority ASC, created_at ASC",
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

pub async fn list_goal_linked_task_ids(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT tg.task_id
         FROM diraigent.task_goal tg
         JOIN diraigent.task t ON tg.task_id = t.id
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
             claimed_at = CASE WHEN state IN ('ready', 'backlog') THEN now() ELSE claimed_at END
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
    if filters.goal_id.is_some() {
        extra_where
            .push_str(" AND id IN (SELECT task_id FROM diraigent.task_goal WHERE goal_id = $9)");
    }
    if filters.unlinked == Some(true) {
        extra_where.push_str(
            " AND NOT EXISTS (SELECT 1 FROM diraigent.task_goal tg WHERE tg.task_id = diraigent.task.id)",
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

    if let Some(goal_id) = filters.goal_id {
        query = query.bind(goal_id);
    }

    let row: (i64,) = query.fetch_one(pool).await?;

    Ok(row.0)
}

#[cfg(test)]
mod tests {
    use super::super::playbooks::create_playbook;
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;

    /// Ephemeral test database helper. Returns (pool, admin_pool, db_name).
    /// Returns None if PostgreSQL is not reachable (test will be skipped).
    async fn setup_test_db() -> Option<(PgPool, PgPool, String)> {
        let db_name = format!("test_validate_ps_{}", Uuid::now_v7().simple());
        let admin_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://zivue:zivue@localhost:5433/diraigent".into());

        let admin_opts = admin_url
            .parse::<sqlx::postgres::PgConnectOptions>()
            .unwrap()
            .ssl_mode(sqlx::postgres::PgSslMode::Disable);

        let admin_pool = match PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(admin_opts)
            .await
        {
            Ok(pool) => pool,
            Err(_) => return None,
        };

        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin_pool)
            .await
            .expect("Failed to create test database");

        let connect_opts = format!("postgres://zivue:zivue@localhost:5433/{db_name}")
            .parse::<sqlx::postgres::PgConnectOptions>()
            .unwrap()
            .ssl_mode(sqlx::postgres::PgSslMode::Disable)
            .options([("search_path", "public,diraigent")]);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(connect_opts)
            .await
            .expect("Failed to connect to test database");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        Some((pool, admin_pool, db_name))
    }

    async fn teardown_test_db(pool: PgPool, admin_pool: PgPool, db_name: &str) {
        pool.close().await;
        let _ = sqlx::query(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{db_name}'"
        ))
        .execute(&admin_pool)
        .await;
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{db_name}\""))
            .execute(&admin_pool)
            .await;
    }

    /// Helper: create a playbook with N steps directly in the database.
    async fn seed_playbook(pool: &PgPool, step_count: usize) -> Uuid {
        let tenant_id: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let created_by = Uuid::now_v7();
        let steps: Vec<serde_json::Value> = (0..step_count)
            .map(|i| serde_json::json!({ "name": format!("step{i}") }))
            .collect();

        let req = CreatePlaybook {
            title: format!("{step_count}-step test playbook"),
            trigger_description: None,
            steps: Some(serde_json::json!(steps)),
            tags: None,
            metadata: None,
            initial_state: None,
        };

        create_playbook(pool, tenant_id, &req, created_by)
            .await
            .expect("seed_playbook")
            .id
    }

    // ── validate_playbook_step tests ──

    #[tokio::test]
    async fn validate_playbook_step_negative_step_is_rejected() {
        let (pool, admin_pool, db_name) = match setup_test_db().await {
            Some(v) => v,
            None => {
                eprintln!("SKIPPED: PostgreSQL not available");
                return;
            }
        };

        let result = validate_playbook_step(&pool, -1, None).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cannot be negative"),
            "expected 'cannot be negative', got: {err_msg}"
        );

        teardown_test_db(pool, admin_pool, &db_name).await;
    }

    #[tokio::test]
    async fn validate_playbook_step_no_playbook_non_negative_is_ok() {
        let (pool, admin_pool, db_name) = match setup_test_db().await {
            Some(v) => v,
            None => {
                eprintln!("SKIPPED: PostgreSQL not available");
                return;
            }
        };

        // With no playbook, any non-negative step should be Ok
        assert!(validate_playbook_step(&pool, 0, None).await.is_ok());
        assert!(validate_playbook_step(&pool, 5, None).await.is_ok());
        assert!(validate_playbook_step(&pool, 100, None).await.is_ok());

        teardown_test_db(pool, admin_pool, &db_name).await;
    }

    #[tokio::test]
    async fn validate_playbook_step_in_bounds_is_ok() {
        let (pool, admin_pool, db_name) = match setup_test_db().await {
            Some(v) => v,
            None => {
                eprintln!("SKIPPED: PostgreSQL not available");
                return;
            }
        };

        let playbook_id = seed_playbook(&pool, 3).await;

        // Steps 0, 1, 2 are all in-bounds for a 3-step playbook
        assert!(
            validate_playbook_step(&pool, 0, Some(playbook_id))
                .await
                .is_ok()
        );
        assert!(
            validate_playbook_step(&pool, 1, Some(playbook_id))
                .await
                .is_ok()
        );
        assert!(
            validate_playbook_step(&pool, 2, Some(playbook_id))
                .await
                .is_ok()
        );

        teardown_test_db(pool, admin_pool, &db_name).await;
    }

    #[tokio::test]
    async fn validate_playbook_step_out_of_bounds_is_rejected() {
        let (pool, admin_pool, db_name) = match setup_test_db().await {
            Some(v) => v,
            None => {
                eprintln!("SKIPPED: PostgreSQL not available");
                return;
            }
        };

        let playbook_id = seed_playbook(&pool, 3).await;

        // Step 3 is out-of-bounds for a 3-step playbook (valid: 0, 1, 2)
        let result = validate_playbook_step(&pool, 3, Some(playbook_id)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("out of range"),
            "expected 'out of range', got: {err_msg}"
        );
        assert!(
            err_msg.contains("3 steps"),
            "expected '3 steps' in message, got: {err_msg}"
        );

        // Much larger step
        let result = validate_playbook_step(&pool, 100, Some(playbook_id)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("out of range"),
            "expected 'out of range', got: {err_msg}"
        );

        teardown_test_db(pool, admin_pool, &db_name).await;
    }

    #[tokio::test]
    async fn validate_playbook_step_negative_with_playbook_is_rejected() {
        let (pool, admin_pool, db_name) = match setup_test_db().await {
            Some(v) => v,
            None => {
                eprintln!("SKIPPED: PostgreSQL not available");
                return;
            }
        };

        let playbook_id = seed_playbook(&pool, 3).await;

        // Negative check fires before the bounds check
        let result = validate_playbook_step(&pool, -1, Some(playbook_id)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cannot be negative"),
            "expected 'cannot be negative', got: {err_msg}"
        );

        teardown_test_db(pool, admin_pool, &db_name).await;
    }
}
