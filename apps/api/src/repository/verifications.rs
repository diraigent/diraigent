use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, fetch_by_id};

pub async fn create_verification(
    pool: &PgPool,
    project_id: Uuid,
    agent_id: Option<Uuid>,
    user_id: Option<Uuid>,
    req: &CreateVerification,
) -> Result<Verification, AppError> {
    let status = req.status.as_deref().unwrap_or("pass");
    let evidence = req
        .evidence
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    sqlx::query_as::<_, Verification>(
        "INSERT INTO diraigent.verification (project_id, task_id, agent_id, user_id, kind, status, title, detail, evidence)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *",
    )
    .bind(project_id)
    .bind(req.task_id)
    .bind(agent_id)
    .bind(user_id)
    .bind(&req.kind)
    .bind(status)
    .bind(&req.title)
    .bind(&req.detail)
    .bind(&evidence)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn list_verifications(
    pool: &PgPool,
    project_id: Uuid,
    filters: &VerificationFilters,
) -> Result<Vec<Verification>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    sqlx::query_as::<_, Verification>(
        "SELECT * FROM diraigent.verification
         WHERE project_id = $1
           AND ($2::uuid IS NULL OR task_id = $2)
           AND ($3::text IS NULL OR kind = $3)
           AND ($4::text IS NULL OR status = $4)
         ORDER BY created_at DESC
         LIMIT $5 OFFSET $6",
    )
    .bind(project_id)
    .bind(filters.task_id)
    .bind(&filters.kind)
    .bind(&filters.status)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn count_verifications(
    pool: &PgPool,
    project_id: Uuid,
    filters: &VerificationFilters,
) -> Result<i64, AppError> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM diraigent.verification
         WHERE project_id = $1
           AND ($2::uuid IS NULL OR task_id = $2)
           AND ($3::text IS NULL OR kind = $3)
           AND ($4::text IS NULL OR status = $4)",
    )
    .bind(project_id)
    .bind(filters.task_id)
    .bind(&filters.kind)
    .bind(&filters.status)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_verification_by_id(pool: &PgPool, id: Uuid) -> Result<Verification, AppError> {
    fetch_by_id(pool, Table::Verification, id, "Verification not found").await
}

pub async fn update_verification(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateVerification,
) -> Result<Verification, AppError> {
    sqlx::query_as::<_, Verification>(
        "UPDATE diraigent.verification SET
            status = COALESCE($2, status),
            detail = COALESCE($3, detail),
            evidence = COALESCE($4, evidence),
            updated_at = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&req.status)
    .bind(&req.detail)
    .bind(&req.evidence)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Verification not found".into()))
}
