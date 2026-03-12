use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

pub async fn create_task_log(
    pool: &PgPool,
    project_id: Uuid,
    agent_id: Option<Uuid>,
    req: &CreateTaskLog,
) -> Result<TaskLog, AppError> {
    let step_name = req.step_name.as_deref().unwrap_or("implement");
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    sqlx::query_as::<_, TaskLog>(
        "INSERT INTO diraigent.task_log (project_id, task_id, agent_id, step_name, content, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(project_id)
    .bind(req.task_id)
    .bind(agent_id)
    .bind(step_name)
    .bind(&req.content)
    .bind(&metadata)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn list_task_logs(
    pool: &PgPool,
    project_id: Uuid,
    filters: &TaskLogFilters,
) -> Result<Vec<TaskLogSummary>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    sqlx::query_as::<_, TaskLogSummary>(
        "SELECT id, task_id, project_id, agent_id, step_name, metadata, created_at
         FROM diraigent.task_log
         WHERE project_id = $1
           AND ($2::uuid IS NULL OR task_id = $2)
           AND ($3::text IS NULL OR step_name = $3)
         ORDER BY created_at DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(project_id)
    .bind(filters.task_id)
    .bind(&filters.step_name)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn count_task_logs(
    pool: &PgPool,
    project_id: Uuid,
    filters: &TaskLogFilters,
) -> Result<i64, AppError> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM diraigent.task_log
         WHERE project_id = $1
           AND ($2::uuid IS NULL OR task_id = $2)
           AND ($3::text IS NULL OR step_name = $3)",
    )
    .bind(project_id)
    .bind(filters.task_id)
    .bind(&filters.step_name)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_task_log_by_id(pool: &PgPool, id: Uuid) -> Result<TaskLog, AppError> {
    sqlx::query_as::<_, TaskLog>("SELECT * FROM diraigent.task_log WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Task log not found".into()))
}
