use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
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
        .route("/{project_id}/observations", post(create).get(list))
        .route("/{project_id}/observations/cleanup", post(cleanup))
        .route(
            "/observations/{id}",
            get(get_one).put(update).delete(delete_observation),
        )
        .route("/observations/{id}/dismiss", post(dismiss))
        .route("/observations/{id}/promote", post(promote))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateObservation>,
) -> Result<Json<Observation>, AppError> {
    let pkg = state.pkg_cache.get_for_project(project_id).await?;
    validation::validate_create_observation(&req, pkg.as_ref())?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let o = state.db.create_observation(project_id, &req).await?;
    Ok(Json(o))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<ObservationFilters>,
) -> Result<Json<PaginatedResponse<Observation>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_observations(project_id, &filters),
        state.db.count_observations(project_id, &filters),
    )
    .await
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Observation>, AppError> {
    let o = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_observation_by_id(id).await?,
    )
    .await?;
    Ok(Json(o))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateObservation>,
) -> Result<Json<Observation>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_observation_by_id(id).await?,
        "execute",
    )
    .await?;
    let o = state.db.update_observation(id, &req).await?;
    Ok(Json(o))
}

async fn dismiss(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Observation>, AppError> {
    let old = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_observation_by_id(id).await?,
        "decide",
    )
    .await?;
    let o = state.db.dismiss_observation(id).await?;

    state.fire_event(
        o.project_id,
        "observation.dismissed",
        "observation",
        o.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"observation_id": o.id, "title": o.title, "from": old.status, "to": o.status}),
    );

    Ok(Json(o))
}

#[derive(Serialize)]
struct PromoteResponse {
    observation: Observation,
    work: Work,
    task: Task,
}

async fn promote(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<PromoteObservation>,
) -> Result<Json<PromoteResponse>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_observation_by_id(id).await?,
        "decide",
    )
    .await?;

    let (observation, work, task) = state.db.promote_observation(id, &req, user_id).await?;

    state.fire_event(
        observation.project_id,
        "observation.promoted",
        "observation",
        observation.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"observation_id": observation.id, "title": observation.title, "work_id": work.id, "task_id": task.id}),
    );

    Ok(Json(PromoteResponse {
        observation,
        work,
        task,
    }))
}

async fn delete_observation(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_observation_by_id(id).await?,
        "manage",
    )
    .await?;
    state.db.delete_observation(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn cleanup(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<CleanupObservationsResult>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let result = state.db.cleanup_observations(project_id).await?;
    Ok(Json(result))
}
