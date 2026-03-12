use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::packages::get_package_by_slug;
use super::{Table, delete_by_id, fetch_by_id, slugify};

// ── Projects ──

pub async fn create_project(
    pool: &PgPool,
    req: &CreateProject,
    owner_id: Uuid,
) -> Result<Project, AppError> {
    let slug = req.slug.clone().unwrap_or_else(|| slugify(&req.name));
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let default_branch = req.default_branch.as_deref().unwrap_or("main").to_string();

    // Auto-assign the "default"-tagged playbook to new projects
    let default_playbook_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM diraigent.playbook WHERE 'default' = ANY(tags) ORDER BY created_at ASC LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    // Resolve package: use given slug, fall back to software-dev
    let package_slug = req.package_slug.as_deref().unwrap_or("software-dev");
    let package_id = get_package_by_slug(pool, package_slug)
        .await
        .map(|p| p.id)
        .ok();

    // New git model fields — with backward compat for legacy repo_path
    let git_mode = req.git_mode.as_deref().unwrap_or("standalone").to_string();
    // git_root defaults to repo_path when the new field is not provided
    let git_root = req.git_root.clone().or_else(|| req.repo_path.clone());
    // repo_path is kept in sync with git_root for backward compat
    let legacy_repo_path = git_root.clone().or_else(|| req.repo_path.clone());

    // Default tenant for projects that don't specify one
    let default_tenant_id: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    let tenant_id = req.tenant_id.unwrap_or(default_tenant_id);

    let project = sqlx::query_as::<_, Project>(
        "INSERT INTO diraigent.project (name, slug, description, owner_id, parent_id, repo_url, repo_path, default_branch, service_name, metadata, default_playbook_id, package_id, git_mode, git_root, project_root, tenant_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
         RETURNING *",
    )
    .bind(&req.name)
    .bind(&slug)
    .bind(&req.description)
    .bind(owner_id)
    .bind(req.parent_id)
    .bind(&req.repo_url)
    .bind(&legacy_repo_path)
    .bind(&default_branch)
    .bind(&req.service_name)
    .bind(&metadata)
    .bind(default_playbook_id)
    .bind(package_id)
    .bind(&git_mode)
    .bind(&git_root)
    .bind(&req.project_root)
    .bind(tenant_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("project_slug_key") => {
            AppError::Conflict(format!("Project with slug '{}' already exists", slug))
        }
        _ => e.into(),
    })?;

    Ok(project)
}

pub async fn get_project_by_id(pool: &PgPool, id: Uuid) -> Result<Project, AppError> {
    fetch_by_id(pool, Table::Project, id, "Project not found").await
}

pub async fn get_project_by_slug(pool: &PgPool, slug: &str) -> Result<Project, AppError> {
    sqlx::query_as::<_, Project>("SELECT * FROM diraigent.project WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Project not found".into()))
}

pub async fn list_projects(pool: &PgPool, p: &Pagination) -> Result<Vec<Project>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM diraigent.project ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(projects)
}

pub async fn list_projects_for_tenant(
    pool: &PgPool,
    tenant_id: Uuid,
    p: &Pagination,
) -> Result<Vec<Project>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let projects = sqlx::query_as::<_, Project>(
        "SELECT * FROM diraigent.project WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(tenant_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(projects)
}

pub async fn update_project(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateProject,
) -> Result<Project, AppError> {
    let existing = get_project_by_id(pool, id).await?;
    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);
    let default_playbook_id = match &req.default_playbook_id {
        Some(val) => *val, // explicitly provided (Some(id) or None to clear)
        None => existing.default_playbook_id, // not provided, keep existing
    };
    let repo_url = match &req.repo_url {
        Some(val) => val.clone(),
        None => existing.repo_url.clone(),
    };
    let repo_path = match &req.repo_path {
        Some(val) => val.clone(),
        None => existing.repo_path.clone(),
    };
    let default_branch = req
        .default_branch
        .as_deref()
        .unwrap_or(&existing.default_branch);
    let service_name = match &req.service_name {
        Some(val) => val.clone(),
        None => existing.service_name.clone(),
    };

    // New git model fields
    let git_mode = req
        .git_mode
        .as_deref()
        .unwrap_or(&existing.git_mode)
        .to_string();
    let git_root = match &req.git_root {
        Some(val) => val.clone(),
        None => existing.git_root.clone(),
    };
    let project_root = match &req.project_root {
        Some(val) => val.clone(),
        None => existing.project_root.clone(),
    };
    // Keep repo_path in sync: if git_root is now set, mirror it; else keep legacy value
    let synced_repo_path = git_root.clone().map(Some).unwrap_or(repo_path.clone());

    // Resolve new package_id if a slug was given, otherwise keep existing
    let package_id = if let Some(slug) = req.package_slug.as_deref() {
        Some(
            get_package_by_slug(pool, slug)
                .await
                .map(|p| p.id)
                .map_err(|_| AppError::Validation(format!("Package '{}' not found", slug)))?,
        )
    } else {
        existing.package_id
    };

    let project = sqlx::query_as::<_, Project>(
        "UPDATE diraigent.project SET name = $2, description = $3, metadata = $4, default_playbook_id = $5,
                repo_url = $6, repo_path = $7, default_branch = $8, service_name = $9, package_id = $10,
                git_mode = $11, git_root = $12, project_root = $13
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(metadata)
    .bind(default_playbook_id)
    .bind(repo_url)
    .bind(synced_repo_path)
    .bind(default_branch)
    .bind(service_name)
    .bind(package_id)
    .bind(git_mode)
    .bind(git_root)
    .bind(project_root)
    .fetch_one(pool)
    .await?;

    Ok(project)
}

pub async fn delete_project(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Project, id, "Project not found").await
}

// ── Project Hierarchy ──

pub async fn get_project_children(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Project>, AppError> {
    let children = sqlx::query_as::<_, Project>(
        "SELECT * FROM diraigent.project WHERE parent_id = $1 ORDER BY name",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(children)
}

pub async fn get_project_tree(pool: &PgPool, root_id: Uuid) -> Result<Vec<Project>, AppError> {
    let tree = sqlx::query_as::<_, Project>(
        "WITH RECURSIVE tree AS (
            SELECT * FROM diraigent.project WHERE id = $1
            UNION ALL
            SELECT p.* FROM diraigent.project p
            JOIN tree t ON p.parent_id = t.id
         )
         SELECT * FROM tree ORDER BY created_at",
    )
    .bind(root_id)
    .fetch_all(pool)
    .await?;
    Ok(tree)
}
