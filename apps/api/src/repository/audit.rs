use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

#[allow(clippy::too_many_arguments)]
pub async fn create_audit_entry(
    pool: &PgPool,
    project_id: Uuid,
    actor_agent_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    action: &str,
    entity_type: &str,
    entity_id: Uuid,
    summary: &str,
    before_state: Option<&serde_json::Value>,
    after_state: Option<&serde_json::Value>,
) -> Result<AuditEntry, AppError> {
    let row = sqlx::query_as::<_, AuditEntry>(
        "INSERT INTO diraigent.audit_log (project_id, actor_agent_id, actor_user_id, action, entity_type, entity_id, summary, before_state, after_state)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *"
    )
    .bind(project_id)
    .bind(actor_agent_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(summary)
    .bind(before_state)
    .bind(after_state)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn list_audit_log(
    pool: &PgPool,
    project_id: Uuid,
    filters: &AuditFilters,
) -> Result<Vec<AuditEntry>, AppError> {
    let limit = filters.limit.unwrap_or(50);
    let offset = filters.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, AuditEntry>(
        "SELECT * FROM diraigent.audit_log
         WHERE project_id = $1
           AND ($2::text IS NULL OR action = $2)
           AND ($3::text IS NULL OR entity_type = $3)
           AND ($4::uuid IS NULL OR entity_id = $4)
           AND ($5::uuid IS NULL OR actor_agent_id = $5)
           AND ($6::timestamptz IS NULL OR created_at >= $6)
         ORDER BY created_at DESC LIMIT $7 OFFSET $8",
    )
    .bind(project_id)
    .bind(&filters.action)
    .bind(&filters.entity_type)
    .bind(filters.entity_id)
    .bind(filters.agent_id)
    .bind(filters.since)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn get_entity_history(
    pool: &PgPool,
    entity_type: &str,
    entity_id: Uuid,
    limit: i64,
) -> Result<Vec<AuditEntry>, AppError> {
    let rows = sqlx::query_as::<_, AuditEntry>(
        "SELECT * FROM diraigent.audit_log WHERE entity_type = $1 AND entity_id = $2 ORDER BY created_at DESC LIMIT $3"
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn count_audit_log(
    pool: &PgPool,
    project_id: Uuid,
    filters: &AuditFilters,
) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM diraigent.audit_log
         WHERE project_id = $1
           AND ($2::text IS NULL OR action = $2)
           AND ($3::text IS NULL OR entity_type = $3)
           AND ($4::uuid IS NULL OR entity_id = $4)
           AND ($5::uuid IS NULL OR actor_agent_id = $5)
           AND ($6::timestamptz IS NULL OR created_at >= $6)",
    )
    .bind(project_id)
    .bind(&filters.action)
    .bind(&filters.entity_type)
    .bind(filters.entity_id)
    .bind(filters.agent_id)
    .bind(filters.since)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}
