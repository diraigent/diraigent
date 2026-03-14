use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::projects::get_project_by_id;
use super::{Table, delete_by_id, fetch_by_id};

// ── Plans ──

pub async fn create_plan(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreatePlan,
    created_by: Uuid,
) -> Result<Plan, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let plan = sqlx::query_as::<_, Plan>(
        "INSERT INTO diraigent.plan (project_id, title, description, metadata, created_by)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(plan)
}

pub async fn get_plan_by_id(pool: &PgPool, id: Uuid) -> Result<Plan, AppError> {
    fetch_by_id(pool, Table::Plan, id, "Plan not found").await
}

pub async fn list_plans(
    pool: &PgPool,
    project_id: Uuid,
    filters: &PlanFilters,
) -> Result<Vec<Plan>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let plans = sqlx::query_as::<_, Plan>(
        "SELECT * FROM diraigent.plan
         WHERE project_id = $1
           AND ($2::text IS NULL OR status = $2)
         ORDER BY created_at DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(project_id)
    .bind(&filters.status)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(plans)
}

pub async fn count_plans(
    pool: &PgPool,
    project_id: Uuid,
    filters: &PlanFilters,
) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM diraigent.plan
         WHERE project_id = $1
           AND ($2::text IS NULL OR status = $2)",
    )
    .bind(project_id)
    .bind(&filters.status)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn update_plan(pool: &PgPool, id: Uuid, req: &UpdatePlan) -> Result<Plan, AppError> {
    let existing = get_plan_by_id(pool, id).await?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let plan = sqlx::query_as::<_, Plan>(
        "UPDATE diraigent.plan
         SET title = $2, description = $3, status = $4, metadata = $5,
             updated_at = now()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(plan)
}

pub async fn delete_plan(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    // Clear plan_id on all tasks belonging to this plan before deleting
    sqlx::query("UPDATE diraigent.task SET plan_id = NULL, plan_position = 0 WHERE plan_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    delete_by_id(pool, Table::Plan, id, "Plan not found").await
}

/// Add a task to a plan, assigning the next position.
pub async fn add_task_to_plan(
    pool: &PgPool,
    plan_id: Uuid,
    task_id: Uuid,
) -> Result<Task, AppError> {
    let _ = get_plan_by_id(pool, plan_id).await?;

    let task = sqlx::query_as::<_, Task>(
        "UPDATE diraigent.task
         SET plan_id = $1,
             plan_position = (SELECT COALESCE(MAX(plan_position), -1) + 1
                              FROM diraigent.task WHERE plan_id = $1),
             updated_at = now()
         WHERE id = $2 RETURNING *",
    )
    .bind(plan_id)
    .bind(task_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".into()))?;

    Ok(task)
}

/// Remove a task from its plan.
pub async fn remove_task_from_plan(
    pool: &PgPool,
    plan_id: Uuid,
    task_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE diraigent.task SET plan_id = NULL, plan_position = 0, updated_at = now()
         WHERE id = $1 AND plan_id = $2",
    )
    .bind(task_id)
    .bind(plan_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Task not found in this plan".into()));
    }
    Ok(())
}

/// List tasks belonging to a plan, ordered by plan_position.
pub async fn list_plan_tasks(
    pool: &PgPool,
    plan_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Task>, AppError> {
    let _ = get_plan_by_id(pool, plan_id).await?;
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT * FROM diraigent.task
         WHERE plan_id = $1
         ORDER BY plan_position ASC, created_at ASC
         LIMIT $2 OFFSET $3",
    )
    .bind(plan_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

/// Count tasks in a plan.
pub async fn count_plan_tasks(pool: &PgPool, plan_id: Uuid) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM diraigent.task WHERE plan_id = $1")
        .bind(plan_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

/// Reorder tasks within a plan. The task_ids array defines the new landing order.
pub async fn reorder_plan_tasks(
    pool: &PgPool,
    plan_id: Uuid,
    task_ids: &[Uuid],
) -> Result<Vec<Task>, AppError> {
    let _ = get_plan_by_id(pool, plan_id).await?;

    if task_ids.is_empty() {
        return Ok(vec![]);
    }

    // Validate all task_ids belong to this plan
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM diraigent.task WHERE id = ANY($1) AND plan_id = $2")
            .bind(task_ids)
            .bind(plan_id)
            .fetch_one(pool)
            .await?;

    if count.0 != task_ids.len() as i64 {
        return Err(AppError::Validation(
            "Some task IDs do not belong to this plan".into(),
        ));
    }

    // Update plan_position for each task
    let positions: Vec<i32> = (0..task_ids.len() as i32).collect();
    sqlx::query(
        "UPDATE diraigent.task SET plan_position = data.new_pos, updated_at = now()
         FROM (SELECT unnest($1::uuid[]) AS id, unnest($2::int[]) AS new_pos) data
         WHERE diraigent.task.id = data.id",
    )
    .bind(task_ids)
    .bind(&positions)
    .execute(pool)
    .await?;

    // Return tasks in new order
    list_plan_tasks(pool, plan_id, 100, 0).await
}

/// Get progress summary for a plan.
pub async fn get_plan_progress(pool: &PgPool, plan_id: Uuid) -> Result<PlanProgress, AppError> {
    let _ = get_plan_by_id(pool, plan_id).await?;

    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT
            COUNT(*)::bigint AS total,
            COUNT(*) FILTER (WHERE state = 'done')::bigint AS done,
            COUNT(*) FILTER (WHERE state = 'cancelled')::bigint AS cancelled,
            COUNT(*) FILTER (WHERE state NOT IN ('backlog','ready','done','cancelled'))::bigint AS working
         FROM diraigent.task
         WHERE plan_id = $1",
    )
    .bind(plan_id)
    .fetch_one(pool)
    .await?;

    Ok(PlanProgress {
        plan_id,
        total_tasks: row.0,
        done_tasks: row.1,
        cancelled_tasks: row.2,
        working_tasks: row.3,
    })
}
