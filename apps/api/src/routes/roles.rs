use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_tenant_manage_authority};
use crate::error::AppError;
use crate::models::*;
use crate::tenant::TenantContext;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/roles", post(create_role).get(list_roles))
        .route(
            "/roles/{role_id}",
            get(get_role).put(update_role).delete(delete_role),
        )
}

async fn create_role(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Json(req): Json<CreateRole>,
) -> Result<Json<Role>, AppError> {
    validation::validate_create_role(&req)?;
    require_tenant_manage_authority(state.db.as_ref(), agent_id, user_id, tenant.tenant_id).await?;
    let role = state.db.create_role(tenant.tenant_id, &req).await?;
    Ok(Json(role))
}

async fn list_roles(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    tenant: TenantContext,
) -> Result<Json<Vec<Role>>, AppError> {
    let roles = state.db.list_roles(tenant.tenant_id).await?;
    Ok(Json(roles))
}

async fn get_role(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    tenant: TenantContext,
    Path(role_id): Path<Uuid>,
) -> Result<Json<Role>, AppError> {
    let role = state.db.get_role(role_id).await?;
    if role.tenant_id != tenant.tenant_id {
        return Err(AppError::NotFound("Role not found".into()));
    }
    Ok(Json(role))
}

async fn update_role(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(role_id): Path<Uuid>,
    Json(req): Json<UpdateRole>,
) -> Result<Json<Role>, AppError> {
    validation::validate_update_role(&req)?;
    require_tenant_manage_authority(state.db.as_ref(), agent_id, user_id, tenant.tenant_id).await?;
    let role = state.db.update_role(role_id, &req).await?;
    Ok(Json(role))
}

async fn delete_role(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(role_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_tenant_manage_authority(state.db.as_ref(), agent_id, user_id, tenant.tenant_id).await?;
    state.db.delete_role(role_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
