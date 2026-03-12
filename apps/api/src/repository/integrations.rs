use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id};

// ── Integrations ──

pub async fn create_integration(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateIntegration,
) -> Result<Integration, AppError> {
    let auth_type = req.auth_type.as_deref().unwrap_or("none");
    let credentials = req.credentials.clone().unwrap_or(serde_json::json!({}));
    let config = req.config.clone().unwrap_or(serde_json::json!({}));
    let capabilities = req.capabilities.clone().unwrap_or_default();

    let row = sqlx::query_as::<_, Integration>(
        "INSERT INTO diraigent.integration (project_id, name, kind, provider, base_url, auth_type, credentials, config, capabilities)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *"
    )
    .bind(project_id)
    .bind(&req.name)
    .bind(&req.kind)
    .bind(&req.provider)
    .bind(&req.base_url)
    .bind(auth_type)
    .bind(&credentials)
    .bind(&config)
    .bind(&capabilities)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn get_integration(pool: &PgPool, id: Uuid) -> Result<Integration, AppError> {
    fetch_by_id(pool, Table::Integration, id, "Integration not found").await
}

pub async fn list_integrations(
    pool: &PgPool,
    project_id: Uuid,
    filters: &IntegrationFilters,
) -> Result<Vec<Integration>, AppError> {
    let limit = filters.limit.unwrap_or(50);
    let offset = filters.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, Integration>(
        "SELECT * FROM diraigent.integration
         WHERE project_id = $1
           AND ($2::text IS NULL OR kind = $2)
           AND ($3::text IS NULL OR provider = $3)
           AND ($4::bool IS NULL OR enabled = $4)
         ORDER BY name LIMIT $5 OFFSET $6",
    )
    .bind(project_id)
    .bind(&filters.kind)
    .bind(&filters.provider)
    .bind(filters.enabled)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn update_integration(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateIntegration,
) -> Result<Integration, AppError> {
    let current = get_integration(pool, id).await?;

    let name = req.name.as_deref().unwrap_or(&current.name);
    let kind = req.kind.as_deref().unwrap_or(&current.kind);
    let base_url = req.base_url.as_deref().or(current.base_url.as_deref());
    let auth_type = req.auth_type.as_deref().unwrap_or(&current.auth_type);
    let credentials = req.credentials.as_ref().unwrap_or(&current.credentials);
    let config = req.config.as_ref().unwrap_or(&current.config);
    let capabilities = req.capabilities.as_ref().unwrap_or(&current.capabilities);
    let enabled = req.enabled.unwrap_or(current.enabled);

    let row = sqlx::query_as::<_, Integration>(
        "UPDATE diraigent.integration
         SET name = $1, kind = $2, base_url = $3, auth_type = $4, credentials = $5, config = $6, capabilities = $7, enabled = $8
         WHERE id = $9
         RETURNING *"
    )
    .bind(name)
    .bind(kind)
    .bind(base_url)
    .bind(auth_type)
    .bind(credentials)
    .bind(config)
    .bind(capabilities)
    .bind(enabled)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn delete_integration(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Integration, id, "Integration not found").await
}

// ── Agent Integration Access ──

pub async fn grant_agent_access(
    pool: &PgPool,
    integration_id: Uuid,
    agent_id: Uuid,
    permissions: &[String],
    granted_by: Option<Uuid>,
) -> Result<AgentIntegration, AppError> {
    let row = sqlx::query_as::<_, AgentIntegration>(
        "INSERT INTO diraigent.agent_integration (agent_id, integration_id, permissions, granted_by)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (agent_id, integration_id) DO UPDATE SET permissions = $3, granted_by = $4, granted_at = now()
         RETURNING *"
    )
    .bind(agent_id)
    .bind(integration_id)
    .bind(permissions)
    .bind(granted_by)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn revoke_agent_access(
    pool: &PgPool,
    integration_id: Uuid,
    agent_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "DELETE FROM diraigent.agent_integration WHERE agent_id = $1 AND integration_id = $2",
    )
    .bind(agent_id)
    .bind(integration_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Access grant not found".into()));
    }
    Ok(())
}

pub async fn list_agent_integrations(
    pool: &PgPool,
    agent_id: Uuid,
) -> Result<Vec<Integration>, AppError> {
    let rows = sqlx::query_as::<_, Integration>(
        "SELECT i.* FROM diraigent.integration i
         JOIN diraigent.agent_integration ai ON ai.integration_id = i.id
         WHERE ai.agent_id = $1 AND i.enabled = true
         ORDER BY i.name",
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn list_integration_agents(
    pool: &PgPool,
    integration_id: Uuid,
) -> Result<Vec<AgentIntegration>, AppError> {
    let rows = sqlx::query_as::<_, AgentIntegration>(
        "SELECT * FROM diraigent.agent_integration WHERE integration_id = $1",
    )
    .bind(integration_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
