use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::Table;
use super::{delete_by_id, fetch_by_id};

const REPORT_FILTERS_WHERE: &str = "WHERE project_id = $1 \
    AND ($2::text IS NULL OR status = $2) \
    AND ($3::text IS NULL OR kind = $3)";

pub async fn create_report(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateReport,
    created_by: Uuid,
) -> Result<Report, AppError> {
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let r = sqlx::query_as::<_, Report>(
        "INSERT INTO diraigent.report (project_id, title, kind, prompt, created_by, metadata)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(&req.kind)
    .bind(&req.prompt)
    .bind(created_by)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(r)
}

pub async fn get_report_by_id(pool: &PgPool, id: Uuid) -> Result<Report, AppError> {
    fetch_by_id(pool, Table::Report, id, "Report not found").await
}

pub async fn list_reports(
    pool: &PgPool,
    project_id: Uuid,
    filters: &ReportFilters,
) -> Result<Vec<Report>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let sql = format!(
        "SELECT * FROM diraigent.report {} ORDER BY created_at DESC LIMIT $4 OFFSET $5",
        REPORT_FILTERS_WHERE
    );
    let items = sqlx::query_as::<_, Report>(&sql)
        .bind(project_id)
        .bind(&filters.status)
        .bind(&filters.kind)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(items)
}

pub async fn count_reports(
    pool: &PgPool,
    project_id: Uuid,
    filters: &ReportFilters,
) -> Result<i64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) FROM diraigent.report {}",
        REPORT_FILTERS_WHERE
    );
    let row: (i64,) = sqlx::query_as(&sql)
        .bind(project_id)
        .bind(&filters.status)
        .bind(&filters.kind)
        .fetch_one(pool)
        .await?;

    Ok(row.0)
}

pub async fn update_report(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateReport,
) -> Result<Report, AppError> {
    let existing = get_report_by_id(pool, id).await?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let result = req.result.as_deref().or(existing.result.as_deref());
    let task_id = req.task_id.or(existing.task_id);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let r = sqlx::query_as::<_, Report>(
        "UPDATE diraigent.report
         SET title = $2, status = $3, result = $4, task_id = $5, metadata = $6
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(status)
    .bind(result)
    .bind(task_id)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(r)
}

pub async fn get_report_by_task_id(
    pool: &PgPool,
    task_id: Uuid,
) -> Result<Option<Report>, AppError> {
    let report =
        sqlx::query_as::<_, Report>("SELECT * FROM diraigent.report WHERE task_id = $1 LIMIT 1")
            .bind(task_id)
            .fetch_optional(pool)
            .await?;

    Ok(report)
}

pub async fn delete_report(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Report, id, "Report not found").await
}
