use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id};

pub async fn create_membership(
    pool: &PgPool,
    tenant_id: Uuid,
    req: &CreateMembership,
) -> Result<Membership, AppError> {
    let config = req.config.clone().unwrap_or(serde_json::json!({}));

    let m = sqlx::query_as::<_, Membership>(
        "INSERT INTO diraigent.membership (tenant_id, agent_id, role_id, config)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (agent_id, role_id) DO UPDATE SET updated_at = now()
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(req.agent_id)
    .bind(req.role_id)
    .bind(&config)
    .fetch_one(pool)
    .await?;

    Ok(m)
}

pub async fn get_membership(pool: &PgPool, id: Uuid) -> Result<Membership, AppError> {
    fetch_by_id(pool, Table::Membership, id, "Membership not found").await
}

pub async fn list_members(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Membership>, AppError> {
    let members = sqlx::query_as::<_, Membership>(
        "SELECT * FROM diraigent.membership WHERE tenant_id = $1 ORDER BY joined_at",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    Ok(members)
}

pub async fn list_tenant_agent_ids(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT DISTINCT agent_id FROM diraigent.membership WHERE tenant_id = $1 AND status = 'active'",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

pub async fn list_agent_memberships(
    pool: &PgPool,
    agent_id: Uuid,
    tenant_id: Option<Uuid>,
) -> Result<Vec<Membership>, AppError> {
    let memberships = if let Some(tid) = tenant_id {
        sqlx::query_as::<_, Membership>(
            "SELECT * FROM diraigent.membership WHERE agent_id = $1 AND tenant_id = $2 ORDER BY joined_at",
        )
        .bind(agent_id)
        .bind(tid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Membership>(
            "SELECT * FROM diraigent.membership WHERE agent_id = $1 ORDER BY joined_at",
        )
        .bind(agent_id)
        .fetch_all(pool)
        .await?
    };
    Ok(memberships)
}

pub async fn update_membership(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateMembership,
) -> Result<Membership, AppError> {
    let existing = get_membership(pool, id).await?;

    if let Some(ref s) = req.status
        && !MEMBERSHIP_STATUSES.contains(&s.as_str())
    {
        return Err(AppError::Validation(format!(
            "Invalid membership status: {}. Valid: {:?}",
            s, MEMBERSHIP_STATUSES
        )));
    }

    let status = req.status.as_deref().unwrap_or(&existing.status);
    let config = req.config.as_ref().unwrap_or(&existing.config);

    let m = sqlx::query_as::<_, Membership>(
        "UPDATE diraigent.membership SET status = $2, config = $3 WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(status)
    .bind(config)
    .fetch_one(pool)
    .await?;

    Ok(m)
}

pub async fn remove_membership(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Membership, id, "Membership not found").await
}

// ── Authority Check ──

pub async fn check_authority(
    pool: &PgPool,
    agent_id: Uuid,
    project_id: Uuid,
    required_authority: &str,
) -> Result<bool, AppError> {
    // Check if agent has any role with the required authority within the project's tenant
    let result = sqlx::query_as::<_, (bool,)>(
        "SELECT EXISTS (
            SELECT 1 FROM diraigent.membership m
            JOIN diraigent.role r ON r.id = m.role_id
            JOIN diraigent.project p ON p.tenant_id = m.tenant_id
            WHERE m.agent_id = $1
              AND p.id = $2
              AND m.status = 'active'
              AND $3 = ANY(r.authorities)
         ) AS has_authority",
    )
    .bind(agent_id)
    .bind(project_id)
    .bind(required_authority)
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}

// ── Tenant Manage Authority Check ──

pub async fn check_tenant_manage_authority(
    pool: &PgPool,
    agent_id: Uuid,
    tenant_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query_as::<_, (bool,)>(
        "SELECT EXISTS (
            SELECT 1 FROM diraigent.membership m
            JOIN diraigent.role r ON r.id = m.role_id
            WHERE m.agent_id = $1
              AND m.tenant_id = $2
              AND m.status = 'active'
              AND 'manage' = ANY(r.authorities)
         ) AS has_manage",
    )
    .bind(agent_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}

// ── Membership Check ──

pub async fn check_membership(
    pool: &PgPool,
    agent_id: Uuid,
    project_id: Uuid,
) -> Result<bool, AppError> {
    // Check if agent has an active membership in the project's tenant
    let result = sqlx::query_as::<_, (bool,)>(
        "SELECT EXISTS (
            SELECT 1 FROM diraigent.membership m
            JOIN diraigent.project p ON p.tenant_id = m.tenant_id
            WHERE m.agent_id = $1 AND p.id = $2 AND m.status = 'active'
         ) AS is_member",
    )
    .bind(agent_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    Ok(result.0)
}
