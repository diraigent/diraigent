use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
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
        .route("/{project_id}/goals", post(create_goal).get(list_goals))
        .route("/{project_id}/goals/reorder", post(reorder_goals))
        .route(
            "/goals/{goal_id}",
            get(get_goal).put(update_goal).delete(delete_goal),
        )
        .route(
            "/goals/{goal_id}/tasks",
            post(link_task).get(list_goal_tasks_handler),
        )
        .route("/goals/{goal_id}/tasks/bulk", post(bulk_link_tasks_handler))
        .route("/goals/{goal_id}/tasks/{task_id}", delete(unlink_task))
        .route("/goals/{goal_id}/progress", get(get_progress))
        .route("/goals/{goal_id}/stats", get(get_stats))
        .route("/goals/{goal_id}/children", get(list_children))
        .route(
            "/goals/{goal_id}/comments",
            post(create_goal_comment).get(list_goal_comments),
        )
}

async fn create_goal(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateGoal>,
) -> Result<Json<Goal>, AppError> {
    validation::validate_create_goal(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "decide").await?;
    let goal = state.db.create_goal(project_id, &req, user_id).await?;
    Ok(Json(goal))
}

async fn list_goals(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<GoalFilters>,
) -> Result<Json<Vec<Goal>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let goals = state.db.list_goals(project_id, &filters).await?;
    Ok(Json(goals))
}

async fn reorder_goals(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ReorderGoals>,
) -> Result<Json<Vec<Goal>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "decide").await?;
    let goals = state.db.reorder_goals(project_id, &req.goal_ids).await?;
    Ok(Json(goals))
}

async fn get_goal(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<Goal>, AppError> {
    let goal = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    Ok(Json(goal))
}

async fn update_goal(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
    Json(req): Json<UpdateGoal>,
) -> Result<Json<Goal>, AppError> {
    validation::validate_update_goal(&req)?;
    let old = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
        "decide",
    )
    .await?;

    // Reject manual status changes on auto-status goals
    if old.auto_status && req.status.is_some() {
        return Err(AppError::Validation(
            "Cannot manually set status on an auto-status goal".into(),
        ));
    }

    let goal = state.db.update_goal(goal_id, &req).await?;

    if goal.status == "achieved" && old.status != "achieved" {
        state.fire_event(
            goal.project_id,
            "goal.achieved",
            "goal",
            goal.id,
            agent_id,
            None,
            serde_json::json!({"goal_id": goal.id, "title": goal.title}),
        );
    }

    Ok(Json(goal))
}

async fn delete_goal(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
        "manage",
    )
    .await?;
    state.db.delete_goal(goal_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn link_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
    Json(req): Json<LinkTaskGoal>,
) -> Result<Json<TaskGoal>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
        "decide",
    )
    .await?;
    let tg = state.db.link_task_goal(goal_id, req.task_id).await?;
    refresh_auto_status_goals(&state, req.task_id, agent_id).await;
    Ok(Json(tg))
}

async fn unlink_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((goal_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
        "decide",
    )
    .await?;
    state.db.unlink_task_goal(goal_id, task_id).await?;
    refresh_auto_status_goals(&state, task_id, agent_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_progress(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<GoalProgress>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    let progress = state.db.get_goal_progress(goal_id).await?;
    Ok(Json(progress))
}

async fn get_stats(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<GoalStats>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    let stats = state.db.get_goal_stats(goal_id).await?;
    Ok(Json(stats))
}

async fn list_children(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<Vec<Goal>>, AppError> {
    let goal = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    let filters = GoalFilters {
        status: None,
        goal_type: None,
        parent_goal_id: Some(goal_id),
        top_level: None,
        limit: Some(100),
        offset: Some(0),
    };
    let children = state.db.list_goals(goal.project_id, &filters).await?;
    Ok(Json(children))
}

#[derive(Debug, serde::Deserialize)]
struct GoalTasksQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_goal_tasks_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
    Query(q): Query<GoalTasksQuery>,
) -> Result<Json<Vec<Task>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = q.offset.unwrap_or(0);
    let tasks = state.db.list_goal_tasks(goal_id, limit, offset).await?;
    Ok(Json(tasks))
}

async fn bulk_link_tasks_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
    Json(req): Json<BulkLinkTasks>,
) -> Result<Json<serde_json::Value>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
        "decide",
    )
    .await?;
    let linked = state.db.bulk_link_tasks(goal_id, &req.task_ids).await?;
    // Refresh auto-status for all affected tasks
    for task_id in &req.task_ids {
        refresh_auto_status_goals(&state, *task_id, agent_id).await;
    }
    Ok(Json(serde_json::json!({ "linked": linked })))
}

// ── Goal Comments ──

async fn create_goal_comment(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
    Json(req): Json<CreateGoalComment>,
) -> Result<Json<GoalComment>, AppError> {
    let goal = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    let comment = state
        .db
        .create_goal_comment(goal_id, &req, Some(user_id))
        .await?;

    state.fire_event(
        goal.project_id,
        "comment.created",
        "goal_comment",
        comment.id,
        agent_id,
        Some(user_id),
        serde_json::json!({
            "goal_id": goal_id,
            "comment_id": comment.id,
            "content": comment.content,
        }),
    );

    Ok(Json(comment))
}

async fn list_goal_comments(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(goal_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<GoalComment>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_goal_by_id(goal_id).await?,
    )
    .await?;
    let comments = state.db.list_goal_comments(goal_id, &pagination).await?;
    Ok(Json(comments))
}

/// For a given task_id, query all linked goals with `auto_status = true`,
/// compute derived status, update if changed, and fire `goal.achieved` event
/// if applicable.
pub(crate) async fn refresh_auto_status_goals(
    state: &AppState,
    task_id: Uuid,
    agent_id: Option<Uuid>,
) {
    let goal_ids = match state.db.list_auto_status_goal_ids_for_task(task_id).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to list auto-status goals");
            return;
        }
    };

    for goal_id in goal_ids {
        let derived = match state.db.compute_auto_status(goal_id).await {
            Ok(Some(s)) => s,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!(goal_id = %goal_id, error = %e, "Failed to compute auto-status");
                continue;
            }
        };

        let goal = match state.db.get_goal_by_id(goal_id).await {
            Ok(g) => g,
            Err(_) => continue,
        };

        if goal.status == derived {
            continue;
        }

        let update = UpdateGoal {
            title: None,
            description: None,
            status: Some(derived.clone()),
            goal_type: None,
            priority: None,
            parent_goal_id: None,
            auto_status: None,
            intent_type: None,
            target_date: None,
            success_criteria: None,
            metadata: None,
            sort_order: None,
        };

        match state.db.update_goal(goal_id, &update).await {
            Ok(updated) => {
                if updated.status == "achieved" && goal.status != "achieved" {
                    state.fire_event(
                        updated.project_id,
                        "goal.achieved",
                        "goal",
                        updated.id,
                        agent_id,
                        None,
                        serde_json::json!({"goal_id": updated.id, "title": updated.title, "auto": true}),
                    );
                }
            }
            Err(e) => {
                tracing::warn!(goal_id = %goal_id, error = %e, "Failed to auto-update goal status");
            }
        }
    }
}
