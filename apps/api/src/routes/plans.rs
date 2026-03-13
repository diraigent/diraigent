use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, ensure_authority_on, ensure_member, require_authority};
use crate::error::AppError;
use crate::models::*;
use crate::validation;

use super::paginate;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/plans", post(create_plan).get(list_plans))
        .route(
            "/plans/{plan_id}",
            get(get_plan).put(update_plan).delete(delete_plan),
        )
        .route(
            "/plans/{plan_id}/tasks",
            post(add_task).get(list_plan_tasks_handler),
        )
        .route("/plans/{plan_id}/tasks/reorder", post(reorder_tasks))
        .route("/plans/{plan_id}/tasks/{task_id}", delete(remove_task))
        .route("/plans/{plan_id}/progress", get(get_progress))
}

async fn create_plan(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreatePlan>,
) -> Result<Json<Plan>, AppError> {
    validation::validate_create_plan(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "create").await?;
    let plan = state.db.create_plan(project_id, &req, user_id).await?;
    Ok(Json(plan))
}

async fn list_plans(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<PlanFilters>,
) -> Result<Json<PaginatedResponse<Plan>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    paginate(
        filters.limit,
        filters.offset,
        state.db.list_plans(project_id, &filters),
        state.db.count_plans(project_id, &filters),
    )
    .await
}

async fn get_plan(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
) -> Result<Json<Plan>, AppError> {
    let plan = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
    )
    .await?;
    Ok(Json(plan))
}

async fn update_plan(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<UpdatePlan>,
) -> Result<Json<Plan>, AppError> {
    validation::validate_update_plan(&req)?;
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
        "create",
    )
    .await?;
    let plan = state.db.update_plan(plan_id, &req).await?;
    Ok(Json(plan))
}

async fn delete_plan(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
        "manage",
    )
    .await?;
    state.db.delete_plan(plan_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<AddTaskToPlan>,
) -> Result<Json<Task>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
        "create",
    )
    .await?;
    let task = state.db.add_task_to_plan(plan_id, req.task_id).await?;
    Ok(Json(task))
}

#[derive(Debug, serde::Deserialize)]
struct PlanTasksQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_plan_tasks_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
    Query(q): Query<PlanTasksQuery>,
) -> Result<Json<PaginatedResponse<Task>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
    )
    .await?;
    paginate(
        q.limit,
        q.offset,
        state.db.list_plan_tasks(
            plan_id,
            q.limit.unwrap_or(50).min(100),
            q.offset.unwrap_or(0),
        ),
        state.db.count_plan_tasks(plan_id),
    )
    .await
}

async fn remove_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((plan_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
        "create",
    )
    .await?;
    state.db.remove_task_from_plan(plan_id, task_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reorder_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<ReorderPlanTasks>,
) -> Result<Json<Vec<Task>>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
        "create",
    )
    .await?;
    let tasks = state.db.reorder_plan_tasks(plan_id, &req.task_ids).await?;
    Ok(Json(tasks))
}

async fn get_progress(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(plan_id): Path<Uuid>,
) -> Result<Json<PlanProgress>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_plan_by_id(plan_id).await?,
    )
    .await?;
    let progress = state.db.get_plan_progress(plan_id).await?;
    Ok(Json(progress))
}
