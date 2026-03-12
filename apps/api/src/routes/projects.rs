use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::models::*;
use crate::tenant::TenantContext;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_project).get(list_projects))
        .route(
            "/{project_id}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/by-slug/{slug}", get(get_project_by_slug))
        .route("/{project_id}/children", get(get_project_children))
        .route("/{project_id}/tree", get(get_project_tree))
}

/// Resolve the package for a project and build a full ProjectResponse.
async fn build_response(state: &AppState, project: Project) -> Result<ProjectResponse, AppError> {
    let package = if let Some(pkg_id) = project.package_id {
        state
            .db
            .get_package_by_id(pkg_id)
            .await
            .ok()
            .map(PackageInfo::from)
    } else {
        None
    };
    Ok(ProjectResponse::new(
        project,
        state.projects_path.as_ref(),
        package,
    ))
}

async fn create_project(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Json(mut req): Json<CreateProject>,
) -> Result<Json<ProjectResponse>, AppError> {
    validation::validate_create_project(&req)?;
    // If nesting under a parent, require manage authority on the parent
    if let Some(parent_id) = req.parent_id {
        require_authority(state.db.as_ref(), agent_id, user_id, parent_id, "manage").await?;
    }
    // Set tenant_id from the caller's tenant context
    req.tenant_id = Some(tenant.tenant_id);
    let project = state.db.create_project(&req, user_id).await?;

    state.fire_event(
        project.id,
        "project.created",
        "project",
        project.id,
        agent_id,
        Some(user_id),
        serde_json::json!({
            "name": project.name,
            "slug": project.slug,
            "repo_url": project.repo_url,
            "git_mode": project.git_mode,
            "git_root": project.git_root,
            "project_root": project.project_root,
            "default_branch": project.default_branch,
        }),
    );

    Ok(Json(build_response(&state, project).await?))
}

async fn list_projects(
    State(state): State<AppState>,
    tenant: TenantContext,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    let projects = state
        .db
        .list_projects_for_tenant(tenant.tenant_id, &pagination)
        .await?;
    let mut responses = Vec::with_capacity(projects.len());
    for p in projects {
        responses.push(build_response(&state, p).await?);
    }
    Ok(Json(responses))
}

async fn get_project(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let project = state.db.get_project_by_id(project_id).await?;
    // Verify the project belongs to the caller's tenant
    if project.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project does not belong to your tenant".into(),
        ));
    }
    Ok(Json(build_response(&state, project).await?))
}

async fn get_project_by_slug(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(slug): Path<String>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = state.db.get_project_by_slug(&slug).await?;
    if project.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project does not belong to your tenant".into(),
        ));
    }
    require_membership(state.db.as_ref(), agent_id, user_id, project.id).await?;
    Ok(Json(build_response(&state, project).await?))
}

async fn update_project(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<UpdateProject>,
) -> Result<Json<ProjectResponse>, AppError> {
    validation::validate_update_project(&req)?;
    // Verify tenant ownership before allowing update
    let existing = state.db.get_project_by_id(project_id).await?;
    if existing.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project does not belong to your tenant".into(),
        ));
    }
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let project = state.db.update_project(project_id, &req).await?;

    state.fire_event(
        project.id,
        "project.updated",
        "project",
        project.id,
        agent_id,
        None,
        serde_json::json!({
            "name": project.name,
            "slug": project.slug,
            "repo_url": project.repo_url,
            "git_mode": project.git_mode,
            "git_root": project.git_root,
            "project_root": project.project_root,
            "default_branch": project.default_branch,
        }),
    );

    Ok(Json(build_response(&state, project).await?))
}

async fn get_project_children(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let existing = state.db.get_project_by_id(project_id).await?;
    if existing.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project does not belong to your tenant".into(),
        ));
    }
    let children = state.db.get_project_children(project_id).await?;
    let mut responses = Vec::with_capacity(children.len());
    for p in children {
        responses.push(build_response(&state, p).await?);
    }
    Ok(Json(responses))
}

async fn get_project_tree(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ProjectResponse>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let existing = state.db.get_project_by_id(project_id).await?;
    if existing.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project does not belong to your tenant".into(),
        ));
    }
    let tree = state.db.get_project_tree(project_id).await?;
    let mut responses = Vec::with_capacity(tree.len());
    for p in tree {
        responses.push(build_response(&state, p).await?);
    }
    Ok(Json(responses))
}

async fn delete_project(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<(), AppError> {
    let existing = state.db.get_project_by_id(project_id).await?;
    if existing.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project does not belong to your tenant".into(),
        ));
    }
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let children = state.db.get_project_children(project_id).await?;
    if !children.is_empty() {
        return Err(AppError::Conflict(
            "Cannot delete a project that has child projects. Delete or reparent the children first.".into(),
        ));
    }
    state.db.delete_project(project_id).await?;
    Ok(())
}
