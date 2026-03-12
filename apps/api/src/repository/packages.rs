use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, fetch_by_id};

pub async fn get_package_by_id(pool: &PgPool, id: Uuid) -> Result<Package, AppError> {
    fetch_by_id(pool, Table::Package, id, "Package not found").await
}

pub async fn get_package_by_slug(pool: &PgPool, slug: &str) -> Result<Package, AppError> {
    sqlx::query_as::<_, Package>("SELECT * FROM diraigent.package WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Package '{}' not found", slug)))
}

/// Fetch the package assigned to a project. Returns None if the project has no
/// package_id yet (e.g. before migration 023 runs).
pub async fn get_package_for_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<Package>, AppError> {
    let pkg = sqlx::query_as::<_, Package>(
        "SELECT p.* FROM diraigent.package p
         JOIN diraigent.project proj ON proj.package_id = p.id
         WHERE proj.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(pkg)
}

pub async fn list_packages(pool: &PgPool) -> Result<Vec<Package>, AppError> {
    let pkgs = sqlx::query_as::<_, Package>(
        "SELECT * FROM diraigent.package ORDER BY is_builtin DESC, name ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(pkgs)
}

pub async fn create_package(pool: &PgPool, req: &CreatePackage) -> Result<Package, AppError> {
    let empty: Vec<String> = vec![];
    let allowed_task_kinds = req.allowed_task_kinds.as_deref().unwrap_or(&empty);
    let allowed_knowledge_categories = req
        .allowed_knowledge_categories
        .as_deref()
        .unwrap_or(&empty);
    let allowed_observation_kinds = req.allowed_observation_kinds.as_deref().unwrap_or(&empty);
    let allowed_event_kinds = req.allowed_event_kinds.as_deref().unwrap_or(&empty);
    let allowed_integration_kinds = req.allowed_integration_kinds.as_deref().unwrap_or(&empty);
    let metadata = req
        .metadata
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    let pkg = sqlx::query_as::<_, Package>(
        "INSERT INTO diraigent.package
             (slug, name, description, is_builtin,
              allowed_task_kinds, allowed_knowledge_categories, allowed_observation_kinds,
              allowed_event_kinds, allowed_integration_kinds, metadata)
         VALUES ($1, $2, $3, false, $4, $5, $6, $7, $8, $9)
         RETURNING *",
    )
    .bind(&req.slug)
    .bind(&req.name)
    .bind(&req.description)
    .bind(allowed_task_kinds)
    .bind(allowed_knowledge_categories)
    .bind(allowed_observation_kinds)
    .bind(allowed_event_kinds)
    .bind(allowed_integration_kinds)
    .bind(metadata)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("package_slug_key") => {
            AppError::Conflict(format!("Package with slug '{}' already exists", req.slug))
        }
        _ => e.into(),
    })?;
    Ok(pkg)
}

pub async fn update_package(
    pool: &PgPool,
    id: Uuid,
    req: &UpdatePackage,
) -> Result<Package, AppError> {
    let existing = get_package_by_id(pool, id).await?;
    // Slug may only be changed for non-builtin packages
    let slug = if let Some(new_slug) = req.slug.as_deref() {
        if existing.is_builtin {
            return Err(AppError::Conflict(
                "Built-in package slugs cannot be changed".into(),
            ));
        }
        new_slug.to_string()
    } else {
        existing.slug.clone()
    };
    let name = req.name.clone().unwrap_or(existing.name);
    let description = req.description.clone().or(existing.description);
    let allowed_task_kinds = req
        .allowed_task_kinds
        .clone()
        .unwrap_or(existing.allowed_task_kinds);
    let allowed_knowledge_categories = req
        .allowed_knowledge_categories
        .clone()
        .unwrap_or(existing.allowed_knowledge_categories);
    let allowed_observation_kinds = req
        .allowed_observation_kinds
        .clone()
        .unwrap_or(existing.allowed_observation_kinds);
    let allowed_event_kinds = req
        .allowed_event_kinds
        .clone()
        .unwrap_or(existing.allowed_event_kinds);
    let allowed_integration_kinds = req
        .allowed_integration_kinds
        .clone()
        .unwrap_or(existing.allowed_integration_kinds);
    let metadata = req.metadata.clone().unwrap_or(existing.metadata);

    let pkg = sqlx::query_as::<_, Package>(
        "UPDATE diraigent.package
         SET slug = $1, name = $2, description = $3,
             allowed_task_kinds = $4, allowed_knowledge_categories = $5,
             allowed_observation_kinds = $6, allowed_event_kinds = $7,
             allowed_integration_kinds = $8, metadata = $9,
             updated_at = NOW()
         WHERE id = $10
         RETURNING *",
    )
    .bind(&slug)
    .bind(&name)
    .bind(&description)
    .bind(&allowed_task_kinds)
    .bind(&allowed_knowledge_categories)
    .bind(&allowed_observation_kinds)
    .bind(&allowed_event_kinds)
    .bind(&allowed_integration_kinds)
    .bind(&metadata)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("package_slug_key") => {
            AppError::Conflict(format!("Package with slug '{}' already exists", slug))
        }
        _ => e.into(),
    })?
    .ok_or_else(|| AppError::NotFound("Package not found".into()))?;
    Ok(pkg)
}

pub async fn delete_package(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let existing = get_package_by_id(pool, id).await?;
    if existing.is_builtin {
        return Err(AppError::Conflict(
            "Built-in packages cannot be deleted".into(),
        ));
    }
    sqlx::query("DELETE FROM diraigent.package WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
