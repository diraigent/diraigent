use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{
    OptionalAgentId, ensure_authority_on, ensure_member, require_authority, require_membership,
};
use crate::error::AppError;
use crate::models::*;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/verifications", post(create).get(list))
        .route("/verifications/{id}", get(get_one).put(update))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateVerification>,
) -> Result<Json<Verification>, AppError> {
    validation::validate_create_verification(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let v = state
        .db
        .create_verification(project_id, &req, agent_id, Some(user_id))
        .await?;
    Ok(Json(v))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<VerificationFilters>,
) -> Result<Json<PaginatedResponse<Verification>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_verifications(project_id, &filters),
        state.db.count_verifications(project_id, &filters),
    )
    .await
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Verification>, AppError> {
    let v = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_verification_by_id(id).await?,
    )
    .await?;
    Ok(Json(v))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateVerification>,
) -> Result<Json<Verification>, AppError> {
    validation::validate_update_verification(&req)?;
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_verification_by_id(id).await?,
        "execute",
    )
    .await?;
    let v = state.db.update_verification(id, &req).await?;
    Ok(Json(v))
}
