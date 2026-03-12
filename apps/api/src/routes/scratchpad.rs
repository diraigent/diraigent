use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/{project_id}/scratchpad",
        get(get_scratchpad).put(upsert_scratchpad),
    )
}

async fn get_scratchpad(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Option<Scratchpad>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let sp = state.db.get_scratchpad(user_id, project_id).await?;
    Ok(Json(sp))
}

async fn upsert_scratchpad(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<UpsertScratchpad>,
) -> Result<Json<Scratchpad>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let sp = state
        .db
        .upsert_scratchpad(user_id, project_id, &req)
        .await?;
    Ok(Json(sp))
}
