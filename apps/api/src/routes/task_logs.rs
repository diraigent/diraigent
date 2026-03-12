use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/task-logs", post(create).get(list))
        .route("/task-logs/{id}", get(get_one))
}

/// Upload a task execution log. Requires `execute` authority on the project.
async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateTaskLog>,
) -> Result<Json<TaskLogSummary>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let log = state.db.create_task_log(project_id, agent_id, &req).await?;
    // Return summary (without content) to avoid echoing the large log body back.
    Ok(Json(TaskLogSummary {
        id: log.id,
        task_id: log.task_id,
        project_id: log.project_id,
        agent_id: log.agent_id,
        step_name: log.step_name,
        metadata: log.metadata,
        created_at: log.created_at,
    }))
}

/// List task log summaries for a project, optionally filtered by task_id and step_name.
async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<TaskLogFilters>,
) -> Result<Json<PaginatedResponse<TaskLogSummary>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_task_logs(project_id, &filters),
        state.db.count_task_logs(project_id, &filters),
    )
    .await
}

/// Get a full task log entry by ID (includes content).
async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskLog>, AppError> {
    let log = state.db.get_task_log_by_id(id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, log.project_id).await?;
    Ok(Json(log))
}
