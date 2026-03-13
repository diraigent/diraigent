use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::projects::get_project_by_id;
use super::{Table, delete_by_id, fetch_by_id};

// ── Goals ──

pub async fn create_goal(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateGoal,
    created_by: Uuid,
) -> Result<Goal, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let success_criteria = req
        .success_criteria
        .clone()
        .unwrap_or(serde_json::json!([]));
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));
    let goal_type = req.goal_type.as_deref().unwrap_or("epic");
    let priority = req.priority.unwrap_or(0);
    let auto_status = req.auto_status.unwrap_or(false);

    let goal = sqlx::query_as::<_, Goal>(
        "INSERT INTO diraigent.goal (project_id, title, description, goal_type, priority, parent_goal_id, auto_status, target_date, success_criteria, metadata, created_by, sort_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                 (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM diraigent.goal WHERE project_id = $1))
         RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(goal_type)
    .bind(priority)
    .bind(req.parent_goal_id)
    .bind(auto_status)
    .bind(req.target_date)
    .bind(&success_criteria)
    .bind(&metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(goal)
}

pub async fn get_goal_by_id(pool: &PgPool, id: Uuid) -> Result<Goal, AppError> {
    fetch_by_id(pool, Table::Goal, id, "Goal not found").await
}

pub async fn list_goals(
    pool: &PgPool,
    project_id: Uuid,
    filters: &GoalFilters,
) -> Result<Vec<Goal>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);
    let top_level = filters.top_level.unwrap_or(false);

    let goals = sqlx::query_as::<_, Goal>(
        "SELECT * FROM diraigent.goal
         WHERE project_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR goal_type = $3)
           AND ($4::uuid IS NULL OR parent_goal_id = $4)
           AND (NOT $5 OR parent_goal_id IS NULL)
         ORDER BY sort_order ASC, created_at DESC
         LIMIT $6 OFFSET $7",
    )
    .bind(project_id)
    .bind(&filters.status)
    .bind(&filters.goal_type)
    .bind(filters.parent_goal_id)
    .bind(top_level)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(goals)
}

pub async fn update_goal(pool: &PgPool, id: Uuid, req: &UpdateGoal) -> Result<Goal, AppError> {
    let existing = get_goal_by_id(pool, id).await?;

    // Self-parent check
    if let Some(Some(parent_id)) = req.parent_goal_id
        && parent_id == id
    {
        return Err(AppError::Validation(
            "A goal cannot be its own parent".into(),
        ));
    }

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let goal_type = req.goal_type.as_deref().unwrap_or(&existing.goal_type);
    let priority = req.priority.unwrap_or(existing.priority);
    let auto_status = req.auto_status.unwrap_or(existing.auto_status);
    let parent_goal_id = match req.parent_goal_id {
        None => existing.parent_goal_id, // no change
        Some(None) => None,              // clear
        Some(Some(pid)) => Some(pid),    // set
    };
    let target_date = req.target_date.or(existing.target_date);
    let success_criteria = req
        .success_criteria
        .as_ref()
        .unwrap_or(&existing.success_criteria);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);

    let goal = sqlx::query_as::<_, Goal>(
        "UPDATE diraigent.goal
         SET title = $2, description = $3, status = $4, goal_type = $5, priority = $6,
             parent_goal_id = $7, auto_status = $8, target_date = $9,
             success_criteria = $10, metadata = $11, sort_order = $12
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(goal_type)
    .bind(priority)
    .bind(parent_goal_id)
    .bind(auto_status)
    .bind(target_date)
    .bind(success_criteria)
    .bind(metadata)
    .bind(sort_order)
    .fetch_one(pool)
    .await?;

    Ok(goal)
}

pub async fn delete_goal(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Goal, id, "Goal not found").await
}

pub async fn link_task_goal(
    pool: &PgPool,
    goal_id: Uuid,
    task_id: Uuid,
) -> Result<TaskGoal, AppError> {
    let tg = sqlx::query_as::<_, TaskGoal>(
        "INSERT INTO diraigent.task_goal (task_id, goal_id) VALUES ($1, $2) RETURNING *",
    )
    .bind(task_id)
    .bind(goal_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Link already exists or invalid reference".into())
        }
        _ => e.into(),
    })?;
    Ok(tg)
}

pub async fn unlink_task_goal(pool: &PgPool, goal_id: Uuid, task_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM diraigent.task_goal WHERE goal_id = $1 AND task_id = $2")
        .bind(goal_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Link not found".into()));
    }
    Ok(())
}

pub async fn get_goal_progress(pool: &PgPool, goal_id: Uuid) -> Result<GoalProgress, AppError> {
    let _ = get_goal_by_id(pool, goal_id).await?;

    let row = sqlx::query_as::<_, (i64, i64)>(
        "WITH RECURSIVE descendants AS (
            SELECT id FROM diraigent.goal WHERE id = $1
            UNION ALL
            SELECT g.id FROM diraigent.goal g JOIN descendants d ON g.parent_goal_id = d.id
        )
        SELECT
            COUNT(DISTINCT tg.task_id)::bigint,
            COUNT(DISTINCT tg.task_id) FILTER (WHERE t.state = 'done')::bigint
         FROM diraigent.task_goal tg
         JOIN diraigent.task t ON t.id = tg.task_id
         WHERE tg.goal_id IN (SELECT id FROM descendants)",
    )
    .bind(goal_id)
    .fetch_one(pool)
    .await?;

    Ok(GoalProgress {
        goal_id,
        total_tasks: row.0,
        done_tasks: row.1,
    })
}

pub async fn get_goal_stats(pool: &PgPool, goal_id: Uuid) -> Result<GoalStats, AppError> {
    let _ = get_goal_by_id(pool, goal_id).await?;

    let row = sqlx::query_as::<_, (
        i64, i64, i64, i64, i64, i64,
        Option<serde_json::Value>,
        Option<f64>, i64, i64,
        i64,
        Option<f64>,
        Option<chrono::DateTime<chrono::Utc>>,
    )>(
        "WITH tasks AS (
            SELECT t.* FROM diraigent.task_goal tg
            JOIN diraigent.task t ON t.id = tg.task_id
            WHERE tg.goal_id = $1
        ),
        blocked AS (
            SELECT DISTINCT td.task_id FROM diraigent.task_dependency td
            JOIN tasks t ON td.task_id = t.id
            JOIN diraigent.task blocker ON td.depends_on = blocker.id
            WHERE blocker.state != 'done'
        ),
        kind_agg AS (
            SELECT COALESCE(jsonb_object_agg(kind, cnt), '{}'::jsonb) AS breakdown
            FROM (SELECT kind, COUNT(*)::bigint AS cnt FROM tasks GROUP BY kind) k
        )
        SELECT
            COUNT(*) FILTER (WHERE state = 'backlog')::bigint AS backlog_count,
            COUNT(*) FILTER (WHERE state = 'ready')::bigint AS ready_count,
            COUNT(*) FILTER (WHERE state NOT IN ('backlog','ready','done','cancelled'))::bigint AS working_count,
            COUNT(*) FILTER (WHERE state = 'done')::bigint AS done_count,
            COUNT(*) FILTER (WHERE state = 'cancelled')::bigint AS cancelled_count,
            COUNT(*)::bigint AS total_count,
            (SELECT breakdown FROM kind_agg) AS kind_breakdown,
            SUM(cost_usd)::double precision AS total_cost_usd,
            COALESCE(SUM(input_tokens)::bigint, 0) AS total_input_tokens,
            COALESCE(SUM(output_tokens)::bigint, 0) AS total_output_tokens,
            (SELECT COUNT(*) FROM blocked)::bigint AS blocked_count,
            EXTRACT(EPOCH FROM AVG(completed_at - created_at)
                FILTER (WHERE completed_at IS NOT NULL))::double precision / 3600.0 AS avg_completion_hours,
            MIN(created_at) FILTER (WHERE state NOT IN ('done','cancelled')) AS oldest_open_task_date
        FROM tasks",
    )
    .bind(goal_id)
    .fetch_one(pool)
    .await?;

    Ok(GoalStats {
        goal_id,
        backlog_count: row.0,
        ready_count: row.1,
        working_count: row.2,
        done_count: row.3,
        cancelled_count: row.4,
        total_count: row.5,
        kind_breakdown: row.6.unwrap_or(serde_json::json!({})),
        total_cost_usd: row.7.unwrap_or(0.0),
        total_input_tokens: row.8,
        total_output_tokens: row.9,
        blocked_count: row.10,
        avg_completion_hours: row.11,
        oldest_open_task_date: row.12,
    })
}

pub async fn compute_auto_status(pool: &PgPool, goal_id: Uuid) -> Result<Option<String>, AppError> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE t.state = 'done') AS done,
            COUNT(*) FILTER (WHERE t.state = 'cancelled') AS cancelled,
            COUNT(*) FILTER (WHERE t.state NOT IN ('backlog','ready','done','cancelled')) AS working
         FROM diraigent.task_goal tg
         JOIN diraigent.task t ON t.id = tg.task_id
         WHERE tg.goal_id = $1",
    )
    .bind(goal_id)
    .fetch_one(pool)
    .await?;

    let (total, done, cancelled, working) = row;
    if total == 0 {
        return Ok(None);
    }
    if done == total {
        return Ok(Some("achieved".to_string()));
    }
    if cancelled == total {
        return Ok(Some("abandoned".to_string()));
    }
    if working > 0 {
        return Ok(Some("active".to_string()));
    }
    // All backlog/ready or mix of done/cancelled but not all one
    Ok(Some("active".to_string()))
}

pub async fn list_goal_tasks(
    pool: &PgPool,
    goal_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Task>, AppError> {
    let _ = get_goal_by_id(pool, goal_id).await?;
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT t.* FROM diraigent.task t
         JOIN diraigent.task_goal tg ON t.id = tg.task_id
         WHERE tg.goal_id = $1
         ORDER BY t.priority DESC, t.created_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(goal_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

pub async fn count_goal_tasks(pool: &PgPool, goal_id: Uuid) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM diraigent.task_goal WHERE goal_id = $1")
        .bind(goal_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn bulk_link_tasks(
    pool: &PgPool,
    goal_id: Uuid,
    task_ids: &[Uuid],
) -> Result<i64, AppError> {
    let _ = get_goal_by_id(pool, goal_id).await?;
    let result = sqlx::query(
        "INSERT INTO diraigent.task_goal (task_id, goal_id)
         SELECT unnest($2::uuid[]), $1
         ON CONFLICT DO NOTHING",
    )
    .bind(goal_id)
    .bind(task_ids)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() as i64)
}

// ── Goal Comments ──

pub async fn create_goal_comment(
    pool: &PgPool,
    goal_id: Uuid,
    req: &CreateGoalComment,
    user_id: Option<Uuid>,
) -> Result<GoalComment, AppError> {
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let comment = sqlx::query_as::<_, GoalComment>(
        "INSERT INTO diraigent.goal_comment (goal_id, agent_id, user_id, content, metadata)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(goal_id)
    .bind(req.agent_id)
    .bind(user_id)
    .bind(&req.content)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(comment)
}

pub async fn list_goal_comments(
    pool: &PgPool,
    goal_id: Uuid,
    p: &Pagination,
) -> Result<Vec<GoalComment>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let comments = sqlx::query_as::<_, GoalComment>(
        "SELECT * FROM diraigent.goal_comment WHERE goal_id = $1
         ORDER BY created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(goal_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(comments)
}

pub async fn list_goals_for_task(pool: &PgPool, task_id: Uuid) -> Result<Vec<Goal>, AppError> {
    let goals = sqlx::query_as::<_, Goal>(
        "SELECT g.* FROM diraigent.goal g
         JOIN diraigent.task_goal tg ON tg.goal_id = g.id
         WHERE tg.task_id = $1
         ORDER BY g.sort_order ASC, g.created_at ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(goals)
}

pub async fn list_auto_status_goal_ids_for_task(
    pool: &PgPool,
    task_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_as::<_, (Uuid,)>(
        "SELECT g.id FROM diraigent.goal g
         JOIN diraigent.task_goal tg ON tg.goal_id = g.id
         WHERE tg.task_id = $1 AND g.auto_status = true",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(|r| r.0).collect())
}

pub async fn reorder_goals(
    pool: &PgPool,
    project_id: Uuid,
    goal_ids: &[Uuid],
) -> Result<Vec<Goal>, AppError> {
    if goal_ids.is_empty() {
        return Ok(vec![]);
    }

    // Validate all goal_ids belong to the given project
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM diraigent.goal WHERE id = ANY($1) AND project_id = $2",
    )
    .bind(goal_ids)
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    if count.0 != goal_ids.len() as i64 {
        return Err(AppError::Validation(
            "Some goal IDs do not belong to this project".into(),
        ));
    }

    // Build sort_order updates: goal_ids[i] gets sort_order = i
    let indexes: Vec<i32> = (0..goal_ids.len() as i32).collect();

    sqlx::query(
        "UPDATE diraigent.goal SET sort_order = data.new_order
         FROM (SELECT unnest($1::uuid[]) AS id, unnest($2::int[]) AS new_order) data
         WHERE diraigent.goal.id = data.id",
    )
    .bind(goal_ids)
    .bind(&indexes)
    .execute(pool)
    .await?;

    // Return the updated goals in new sort order
    let goals = sqlx::query_as::<_, Goal>(
        "SELECT * FROM diraigent.goal WHERE project_id = $1 ORDER BY sort_order ASC, created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(goals)
}

/// Return all goal IDs linked to a task (no auto_status filter).
pub async fn get_goal_ids_for_task(pool: &PgPool, task_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_as::<_, (Uuid,)>(
        "SELECT tg.goal_id FROM diraigent.task_goal tg WHERE tg.task_id = $1",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(|r| r.0).collect())
}
