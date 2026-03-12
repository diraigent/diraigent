use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{
    OptionalAgentId, ensure_authority_on, ensure_member, require_authority, require_membership,
};
use crate::error::AppError;
use crate::models::{CreateIntegration, GrantAccess, IntegrationFilters, UpdateIntegration};
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/integrations", post(create_integration))
        .route("/{project_id}/integrations", get(list_integrations))
        .route("/integrations/{id}", get(get_integration))
        .route("/integrations/{id}", put(update_integration))
        .route("/integrations/{id}", delete(delete_integration))
        .route("/integrations/{id}/access", post(grant_access))
        .route("/integrations/{id}/access", get(list_access))
        .route(
            "/integrations/{id}/access/{agent_id}",
            delete(revoke_access),
        )
        .route("/agents/{agent_id}/integrations", get(agent_integrations))
}

async fn create_integration(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateIntegration>,
) -> Result<(StatusCode, Json<crate::models::Integration>), AppError> {
    let pkg = state.pkg_cache.get_for_project(project_id).await?;
    validation::validate_create_integration(&body, pkg.as_ref())?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let integration = state.db.create_integration(project_id, &body).await?;
    Ok((StatusCode::CREATED, Json(integration)))
}

async fn list_integrations(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<IntegrationFilters>,
) -> Result<Json<Vec<crate::models::Integration>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let items = state.db.list_integrations(project_id, &filters).await?;
    Ok(Json(items))
}

async fn get_integration(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::models::Integration>, AppError> {
    let item = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_integration(id).await?,
    )
    .await?;
    Ok(Json(item))
}

async fn update_integration(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateIntegration>,
) -> Result<Json<crate::models::Integration>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_integration(id).await?,
        "manage",
    )
    .await?;
    let item = state.db.update_integration(id, &body).await?;
    Ok(Json(item))
}

async fn delete_integration(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_integration(id).await?,
        "manage",
    )
    .await?;
    state.db.delete_integration(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn grant_access(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(integration_id): Path<Uuid>,
    Json(body): Json<GrantAccess>,
) -> Result<(StatusCode, Json<crate::models::AgentIntegration>), AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_integration(integration_id).await?,
        "manage",
    )
    .await?;
    let permissions = body.permissions.unwrap_or_default();
    let access = state
        .db
        .grant_agent_access(integration_id, body.agent_id, permissions)
        .await?;
    Ok((StatusCode::CREATED, Json(access)))
}

async fn list_access(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(integration_id): Path<Uuid>,
) -> Result<Json<Vec<crate::models::AgentIntegration>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_integration(integration_id).await?,
    )
    .await?;
    let items = state.db.list_integration_agents(integration_id).await?;
    Ok(Json(items))
}

async fn revoke_access(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((integration_id, target_agent_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_integration(integration_id).await?,
        "manage",
    )
    .await?;
    state
        .db
        .revoke_agent_access(integration_id, target_agent_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn agent_integrations(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Vec<crate::models::Integration>>, AppError> {
    let is_owner = state
        .db
        .verify_agent_owner(agent_id, user_id)
        .await
        .map_err(|_| AppError::Unauthorized("Agent ownership check failed".into()))?;
    if !is_owner {
        return Err(AppError::Forbidden("You do not own this agent".into()));
    }
    let items = state.db.list_agent_integrations(agent_id).await?;
    Ok(Json(items))
}
