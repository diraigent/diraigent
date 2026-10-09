use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/tasks/{task_id}/changed-files",
            get(list_changed_files).post(create_changed_files),
        )
        .route(
            "/tasks/{task_id}/changed-files/{file_id}",
            get(get_changed_file),
        )
}

async fn list_changed_files(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<ChangedFileSummary>>, AppError> {
    let task = state.db.get_task_by_id(task_id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, task.project_id).await?;
    let files = state.db.list_changed_files(task_id).await?;
    Ok(Json(files))
}

async fn create_changed_files(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<CreateChangedFiles>,
) -> Result<Json<Vec<ChangedFileSummary>>, AppError> {
    let task = state.db.get_task_by_id(task_id).await?;
    require_authority(
        state.db.as_ref(),
        agent_id,
        user_id,
        task.project_id,
        "execute",
    )
    .await?;
    let files = crate::project_content::create_files(&state, task_id, &req).await?;
    Ok(Json(files))
}

async fn get_changed_file(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((task_id, file_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ChangedFile>, AppError> {
    let task = state.db.get_task_by_id(task_id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, task.project_id).await?;
    let mut file = state.db.get_changed_file_by_id(file_id).await?;
    if file.task_id != task_id {
        return Err(AppError::NotFound("Changed file not found".into()));
    }
    if let Some(v) = crate::project_content::get(
        &state,
        diraigent_types::project_content::ContentKind::Diff,
        file_id,
    )
    .await?
    {
        file.diff = v["diff"].as_str().map(str::to_owned);
    }
    Ok(Json(file))
}
