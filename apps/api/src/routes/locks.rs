use axum::extract::{Path, State};
use axum::routing::{delete, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/locks", post(acquire_locks).get(list_locks))
        .route("/{project_id}/locks/{task_id}", delete(release_locks))
}

async fn acquire_locks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<AcquireLocks>,
) -> Result<Json<Vec<FileLock>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    if req.paths.is_empty() {
        return Err(AppError::Validation("paths must be non-empty".into()));
    }
    for p in &req.paths {
        if p.is_empty() {
            return Err(AppError::Validation(
                "Each path glob must be non-empty".into(),
            ));
        }
    }

    // Resolve the agent that will own the lock
    let lock_agent = agent_id
        .ok_or_else(|| AppError::Validation("Agent ID is required to acquire locks".into()))?;

    let locks = state
        .db
        .acquire_file_locks(project_id, req.task_id, &req.paths, lock_agent)
        .await?;

    Ok(Json(locks))
}

async fn list_locks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<FileLock>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let locks = state.db.list_file_locks(project_id).await?;
    Ok(Json(locks))
}

async fn release_locks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let released = state.db.release_file_locks(project_id, task_id).await?;
    Ok(Json(serde_json::json!({ "released": released })))
}
