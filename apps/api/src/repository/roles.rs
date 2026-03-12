use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id};

pub async fn create_role(
    pool: &PgPool,
    tenant_id: Uuid,
    req: &CreateRole,
) -> Result<Role, AppError> {
    let authorities = req.authorities.clone().unwrap_or_default();
    let required_capabilities = req.required_capabilities.clone().unwrap_or_default();
    let knowledge_scope = req.knowledge_scope.clone().unwrap_or_default();
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let role = sqlx::query_as::<_, Role>(
        "INSERT INTO diraigent.role (tenant_id, name, description, authorities, required_capabilities, knowledge_scope, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
    )
    .bind(tenant_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&authorities)
    .bind(&required_capabilities)
    .bind(&knowledge_scope)
    .bind(&metadata)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict(format!("Role '{}' already exists in this tenant", req.name))
        }
        _ => e.into(),
    })?;

    Ok(role)
}

pub async fn get_role(pool: &PgPool, id: Uuid) -> Result<Role, AppError> {
    fetch_by_id(pool, Table::Role, id, "Role not found").await
}

pub async fn list_roles(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Role>, AppError> {
    let roles = sqlx::query_as::<_, Role>(
        "SELECT * FROM diraigent.role WHERE tenant_id = $1 ORDER BY name",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await?;
    Ok(roles)
}

pub async fn update_role(pool: &PgPool, id: Uuid, req: &UpdateRole) -> Result<Role, AppError> {
    let existing = get_role(pool, id).await?;
    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let authorities = req.authorities.as_ref().unwrap_or(&existing.authorities);
    let required_capabilities = req
        .required_capabilities
        .as_ref()
        .unwrap_or(&existing.required_capabilities);
    let knowledge_scope = req
        .knowledge_scope
        .as_ref()
        .unwrap_or(&existing.knowledge_scope);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let role = sqlx::query_as::<_, Role>(
        "UPDATE diraigent.role
         SET name = $2, description = $3, authorities = $4, required_capabilities = $5, knowledge_scope = $6, metadata = $7
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(authorities)
    .bind(required_capabilities)
    .bind(knowledge_scope)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(role)
}

pub async fn delete_role(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Role, id, "Role not found").await
}
