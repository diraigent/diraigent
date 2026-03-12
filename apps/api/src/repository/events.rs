use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::Table;
use super::fetch_by_id;
use super::projects::get_project_by_id;

const EVENT_FILTERS_WHERE: &str = "WHERE project_id = $1 \
    AND ($2::text IS NULL OR kind = $2) \
    AND ($3::text IS NULL OR severity = $3) \
    AND ($4::timestamptz IS NULL OR created_at >= $4)";

// ── Events ──

pub async fn create_event(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateEvent,
) -> Result<Event, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let severity = req.severity.as_deref().unwrap_or("info");
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let e = sqlx::query_as::<_, Event>(
        "INSERT INTO diraigent.event (project_id, kind, source, title, description, severity, metadata, related_task_id, agent_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
    )
    .bind(project_id)
    .bind(&req.kind)
    .bind(&req.source)
    .bind(&req.title)
    .bind(&req.description)
    .bind(severity)
    .bind(&metadata)
    .bind(req.related_task_id)
    .bind(req.agent_id)
    .fetch_one(pool)
    .await?;

    Ok(e)
}

pub async fn get_event_by_id(pool: &PgPool, id: Uuid) -> Result<Event, AppError> {
    fetch_by_id(pool, Table::Event, id, "Event not found").await
}

pub async fn list_events(
    pool: &PgPool,
    project_id: Uuid,
    filters: &EventFilters,
) -> Result<Vec<Event>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let sql = format!(
        "SELECT * FROM diraigent.event {} ORDER BY created_at DESC LIMIT $5 OFFSET $6",
        EVENT_FILTERS_WHERE
    );
    let items = sqlx::query_as::<_, Event>(&sql)
        .bind(project_id)
        .bind(&filters.kind)
        .bind(&filters.severity)
        .bind(filters.since)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(items)
}

pub async fn list_recent_events(
    pool: &PgPool,
    project_id: Uuid,
    count: i64,
) -> Result<Vec<Event>, AppError> {
    let items = sqlx::query_as::<_, Event>(
        "SELECT * FROM diraigent.event WHERE project_id = $1
         ORDER BY created_at DESC LIMIT $2",
    )
    .bind(project_id)
    .bind(count)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn count_events(
    pool: &PgPool,
    project_id: Uuid,
    filters: &EventFilters,
) -> Result<i64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) FROM diraigent.event {}",
        EVENT_FILTERS_WHERE
    );
    let row: (i64,) = sqlx::query_as(&sql)
        .bind(project_id)
        .bind(&filters.kind)
        .bind(&filters.severity)
        .bind(filters.since)
        .fetch_one(pool)
        .await?;

    Ok(row.0)
}
