use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority};
use crate::error::AppError;
use crate::models::*;
use crate::tenant::TenantContext;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/step-templates", post(create).get(list))
        .route(
            "/{project_id}/step-templates/{id}",
            get(get_one).put(update).delete(remove),
        )
        .route("/{project_id}/step-templates/{id}/fork", post(fork))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateStepTemplate>,
) -> Result<Json<StepTemplate>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let t = state
        .db
        .create_step_template(tenant.tenant_id, &req, user_id)
        .await?;
    Ok(Json(t))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<StepTemplateFilters>,
) -> Result<Json<Vec<StepTemplate>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let items = state
        .db
        .list_step_templates(tenant.tenant_id, &filters)
        .await?;
    Ok(Json(items))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<StepTemplate>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let t = state.db.get_step_template_by_id(id).await?;
    // Global (shared) templates are readable by all; tenant-owned must match.
    if let Some(tid) = t.tenant_id
        && tid != tenant.tenant_id
    {
        return Err(AppError::Forbidden(
            "Step template belongs to another tenant".into(),
        ));
    }
    Ok(Json(t))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateStepTemplate>,
) -> Result<Json<StepTemplate>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let existing = state.db.get_step_template_by_id(id).await?;
    if existing.tenant_id.is_none() {
        return Err(AppError::Forbidden(
            "Global step templates are immutable — fork them instead".to_string(),
        ));
    }
    if let Some(tid) = existing.tenant_id
        && tid != tenant.tenant_id
    {
        return Err(AppError::Forbidden(
            "Step template belongs to another tenant".into(),
        ));
    }
    let t = state
        .db
        .update_step_template(id, tenant.tenant_id, &req)
        .await?;
    Ok(Json(t))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let existing = state.db.get_step_template_by_id(id).await?;
    if existing.tenant_id.is_none() {
        return Err(AppError::Forbidden(
            "Global step templates are immutable and cannot be deleted".to_string(),
        ));
    }
    if let Some(tid) = existing.tenant_id
        && tid != tenant.tenant_id
    {
        return Err(AppError::Forbidden(
            "Step template belongs to another tenant".into(),
        ));
    }
    state.db.delete_step_template(id, tenant.tenant_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn fork(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<StepTemplate>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "create").await?;
    let source = state.db.get_step_template_by_id(id).await?;
    // Allow forking global templates and own tenant templates
    if let Some(tid) = source.tenant_id
        && tid != tenant.tenant_id
    {
        return Err(AppError::Forbidden(
            "Step template belongs to another tenant".into(),
        ));
    }
    let empty_req = UpdateStepTemplate::default();
    let t = state
        .db
        .fork_step_template(id, tenant.tenant_id, &empty_req, user_id)
        .await?;
    Ok(Json(t))
}
