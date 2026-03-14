use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use tracing::warn;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;
use crate::event_triggers;
use crate::models::*;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/events", post(create).get(list))
        .route("/{project_id}/events/recent", get(recent))
        .route("/events/{id}", get(get_one))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateEvent>,
) -> Result<Json<Event>, AppError> {
    let pkg = state.pkg_cache.get_for_project(project_id).await?;
    validation::validate_create_event(&req, pkg.as_ref())?;
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let e = state.db.create_event(project_id, &req).await?;

    state.fire_event(
        project_id,
        "event.created",
        "event",
        e.id,
        agent_id,
        None,
        serde_json::json!({"event_id": e.id, "kind": e.kind, "source": e.source}),
    );

    // Best-effort: process event trigger rules to auto-create observations.
    if let Err(err) = event_triggers::process_event_triggers(&state, project_id, &e).await {
        warn!(
            event_id = %e.id,
            project_id = %project_id,
            error = %err,
            "Failed to process event triggers"
        );
    }

    Ok(Json(e))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<EventFilters>,
) -> Result<Json<PaginatedResponse<Event>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_events(project_id, &filters),
        state.db.count_events(project_id, &filters),
    )
    .await
}

async fn recent(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Event>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let items = state.db.list_recent_events(project_id, 20).await?;
    Ok(Json(items))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Event>, AppError> {
    let e = state.db.get_event_by_id(id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, e.project_id).await?;
    Ok(Json(e))
}
