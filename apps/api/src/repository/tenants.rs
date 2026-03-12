use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id, slugify};

pub async fn create_tenant(pool: &PgPool, req: &CreateTenant) -> Result<Tenant, AppError> {
    let slug = req.slug.clone().unwrap_or_else(|| slugify(&req.name));
    sqlx::query_as::<_, Tenant>(
        "INSERT INTO diraigent.tenant (name, slug) VALUES ($1, $2) RETURNING *",
    )
    .bind(&req.name)
    .bind(&slug)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("tenant_slug_key") => {
            AppError::Conflict(format!("Tenant with slug '{}' already exists", slug))
        }
        _ => e.into(),
    })
}

pub async fn get_tenant_by_id(pool: &PgPool, id: Uuid) -> Result<Tenant, AppError> {
    fetch_by_id(pool, Table::Tenant, id, "Tenant not found").await
}

pub async fn get_tenant_by_slug(pool: &PgPool, slug: &str) -> Result<Tenant, AppError> {
    sqlx::query_as::<_, Tenant>("SELECT * FROM diraigent.tenant WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Tenant not found".into()))
}

pub async fn list_tenants(pool: &PgPool, filters: &TenantFilters) -> Result<Vec<Tenant>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);
    Ok(sqlx::query_as::<_, Tenant>(
        "SELECT * FROM diraigent.tenant ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?)
}

pub async fn update_tenant(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateTenant,
) -> Result<Tenant, AppError> {
    let existing = get_tenant_by_id(pool, id).await?;
    let name = req.name.as_deref().unwrap_or(&existing.name);
    let encryption_mode = req
        .encryption_mode
        .as_deref()
        .unwrap_or(&existing.encryption_mode);
    let key_salt = req.key_salt.as_deref().or(existing.key_salt.as_deref());
    let theme_preference = req
        .theme_preference
        .as_deref()
        .unwrap_or(&existing.theme_preference);
    let accent_color = req
        .accent_color
        .as_deref()
        .unwrap_or(&existing.accent_color);

    sqlx::query_as::<_, Tenant>(
        "UPDATE diraigent.tenant SET name = $2, encryption_mode = $3, key_salt = $4,
         theme_preference = $5, accent_color = $6, updated_at = now()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(encryption_mode)
    .bind(key_salt)
    .bind(theme_preference)
    .bind(accent_color)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn delete_tenant(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Tenant, id, "Tenant not found").await
}

// ── Tenant Members ──

pub async fn add_tenant_member(
    pool: &PgPool,
    tenant_id: Uuid,
    req: &AddTenantMember,
) -> Result<TenantMember, AppError> {
    let role = req.role.as_deref().unwrap_or("member");
    sqlx::query_as::<_, TenantMember>(
        "INSERT INTO diraigent.tenant_member (tenant_id, user_id, role)
         VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(tenant_id)
    .bind(req.user_id)
    .bind(role)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn list_tenant_members(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<TenantMember>, AppError> {
    Ok(sqlx::query_as::<_, TenantMember>(
        "SELECT * FROM diraigent.tenant_member WHERE tenant_id = $1 ORDER BY created_at",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await?)
}

pub async fn update_tenant_member(
    pool: &PgPool,
    member_id: Uuid,
    req: &UpdateTenantMember,
) -> Result<TenantMember, AppError> {
    let existing: TenantMember = fetch_by_id(
        pool,
        Table::TenantMember,
        member_id,
        "Tenant member not found",
    )
    .await?;
    let role = req.role.as_deref().unwrap_or(&existing.role);
    sqlx::query_as::<_, TenantMember>(
        "UPDATE diraigent.tenant_member SET role = $2, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(member_id)
    .bind(role)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn remove_tenant_member(pool: &PgPool, member_id: Uuid) -> Result<(), AppError> {
    delete_by_id(
        pool,
        Table::TenantMember,
        member_id,
        "Tenant member not found",
    )
    .await
}

pub async fn get_tenant_member_for_user(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<TenantMember>, AppError> {
    Ok(sqlx::query_as::<_, TenantMember>(
        "SELECT * FROM diraigent.tenant_member WHERE tenant_id = $1 AND user_id = $2 LIMIT 1",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_tenant_for_user(pool: &PgPool, user_id: Uuid) -> Result<Option<Tenant>, AppError> {
    Ok(sqlx::query_as::<_, Tenant>(
        "SELECT t.* FROM diraigent.tenant t
         JOIN diraigent.tenant_member tm ON t.id = tm.tenant_id
         WHERE tm.user_id = $1
         ORDER BY tm.created_at ASC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}
