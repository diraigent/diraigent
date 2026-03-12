use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
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
        .route("/{project_id}/decisions", post(create).get(list))
        .route("/decisions/{id}", get(get_one).put(update).delete(remove))
        .route("/decisions/{id}/tasks", get(list_linked_tasks))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateDecision>,
) -> Result<Json<Decision>, AppError> {
    validation::validate_create_decision(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "decide").await?;
    let d = state.db.create_decision(project_id, &req, user_id).await?;
    Ok(Json(d))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<DecisionFilters>,
) -> Result<Json<PaginatedResponse<Decision>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_decisions(project_id, &filters),
        state.db.count_decisions(project_id, &filters),
    )
    .await
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Decision>, AppError> {
    let d = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_decision_by_id(id).await?,
    )
    .await?;
    Ok(Json(d))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDecision>,
) -> Result<Json<Decision>, AppError> {
    validation::validate_update_decision(&req)?;
    let old = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_decision_by_id(id).await?,
        "decide",
    )
    .await?;

    let d = state.db.update_decision(id, &req).await?;

    if req.status.is_some() && old.status != d.status {
        let event_type = match d.status.as_str() {
            "accepted" => "decision.accepted",
            "rejected" => "decision.rejected",
            _ => "decision.updated",
        };
        state.fire_event(
            d.project_id,
            event_type,
            "decision",
            d.id,
            agent_id,
            Some(user_id),
            serde_json::json!({"decision_id": d.id, "title": d.title, "from": old.status, "to": d.status}),
        );
    }

    Ok(Json(d))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_decision_by_id(id).await?,
        "manage",
    )
    .await?;
    state.db.delete_decision(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_linked_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TaskSummaryForDecision>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_decision_by_id(id).await?,
    )
    .await?;
    let tasks = state.db.list_tasks_by_decision(id).await?;
    Ok(Json(tasks))
}
