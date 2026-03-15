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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/members", post(create_membership).get(list_members))
        .route(
            "/members/{membership_id}",
            get(get_membership)
                .put(update_membership)
                .delete(remove_membership),
        )
        .route(
            "/agents/{agent_id}/memberships",
            get(list_agent_memberships),
        )
}

async fn create_membership(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Json(req): Json<CreateMembership>,
) -> Result<Json<Membership>, AppError> {
    require_tenant_manage_authority(state.db.as_ref(), agent_id, user_id, tenant.tenant_id).await?;
    let m = state.db.create_membership(tenant.tenant_id, &req).await?;
    Ok(Json(m))
}

async fn list_members(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    tenant: TenantContext,
) -> Result<Json<Vec<Membership>>, AppError> {
    let members = state.db.list_members(tenant.tenant_id).await?;
    Ok(Json(members))
}

async fn get_membership(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    tenant: TenantContext,
    Path(membership_id): Path<Uuid>,
) -> Result<Json<Membership>, AppError> {
    let m = state.db.get_membership(membership_id).await?;
    if m.tenant_id != tenant.tenant_id {
        return Err(AppError::NotFound("Membership not found".into()));
    }
    Ok(Json(m))
}

async fn update_membership(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(membership_id): Path<Uuid>,
    Json(req): Json<UpdateMembership>,
) -> Result<Json<Membership>, AppError> {
    let existing = state.db.get_membership(membership_id).await?;
    require_tenant_manage_authority(state.db.as_ref(), agent_id, user_id, existing.tenant_id)
        .await?;
    let m = state.db.update_membership(membership_id, &req).await?;
    Ok(Json(m))
}

async fn remove_membership(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(membership_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let existing = state.db.get_membership(membership_id).await?;
    require_tenant_manage_authority(state.db.as_ref(), agent_id, user_id, existing.tenant_id)
        .await?;
    state.db.remove_membership(membership_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_agent_memberships(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Vec<Membership>>, AppError> {
    // Verify the caller owns the agent whose memberships are being listed.
    let is_owner = state
        .db
        .verify_agent_owner(agent_id, user_id)
        .await
        .map_err(|_| AppError::Unauthorized("Agent ownership check failed".into()))?;
    if !is_owner {
        return Err(AppError::Forbidden("You do not own this agent".into()));
    }
    let memberships = state.db.list_agent_memberships(agent_id, None).await?;
    Ok(Json(memberships))
}
