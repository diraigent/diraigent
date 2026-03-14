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
        .route("/{project_id}/work", post(create_work).get(list_works))
        .route("/{project_id}/work/reorder", post(reorder_works))
        .route(
            "/work/{work_id}",
            get(get_work).put(update_work).delete(delete_work),
        )
        .route(
            "/work/{work_id}/tasks",
            post(link_task).get(list_work_tasks_handler),
        )
        .route("/work/{work_id}/tasks/bulk", post(bulk_link_tasks_handler))
        .route(
            "/{project_id}/work/{work_id}/tasks/reorder",
            post(reorder_work_tasks_handler),
        )
        .route("/work/{work_id}/tasks/{task_id}", delete(unlink_task))
        .route("/work/{work_id}/progress", get(get_progress))
        .route("/work/{work_id}/stats", get(get_stats))
        .route("/work/{work_id}/children", get(list_children))
        .route("/{project_id}/work/{work_id}/activate", post(activate_work))
        .route("/{project_id}/work/{work_id}/plan", post(plan_work))
        .route(
            "/work/{work_id}/comments",
            post(create_work_comment).get(list_work_comments),
        )
}

async fn create_work(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateWork>,
) -> Result<Json<Work>, AppError> {
    validation::validate_create_work(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "decide").await?;
    let work = state.db.create_work(project_id, &req, user_id).await?;
    Ok(Json(work))
}

async fn list_works(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<WorkFilters>,
) -> Result<Json<Vec<Work>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let works = state.db.list_works(project_id, &filters).await?;
    Ok(Json(works))
}

async fn reorder_works(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ReorderWorks>,
) -> Result<Json<Vec<Work>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "decide").await?;
    let works = state.db.reorder_works(project_id, &req.work_ids).await?;
    Ok(Json(works))
}

async fn get_work(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
) -> Result<Json<Work>, AppError> {
    let work = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    Ok(Json(work))
}

async fn update_work(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
    Json(req): Json<UpdateWork>,
) -> Result<Json<Work>, AppError> {
    validation::validate_update_work(&req)?;
    let old = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
        "decide",
    )
    .await?;

    // Reject manual status changes on auto-status work items
    if old.auto_status && req.status.is_some() {
        return Err(AppError::Validation(
            "Cannot manually set status on an auto-status work item".into(),
        ));
    }

    let work = state.db.update_work(work_id, &req).await?;

    if work.status == "achieved" && old.status != "achieved" {
        state.fire_event(
            work.project_id,
            "work.achieved",
            "work",
            work.id,
            agent_id,
            None,
            serde_json::json!({"work_id": work.id, "title": work.title}),
        );
    }

    Ok(Json(work))
}

async fn delete_work(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
        "manage",
    )
    .await?;
    state.db.delete_work(work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn activate_work(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, work_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Work>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    // Verify the work item belongs to this project
    let existing = state.db.get_work_by_id(work_id).await?;
    if existing.project_id != project_id {
        return Err(AppError::NotFound("Work item not found".into()));
    }

    let work = state.db.activate_work(work_id).await?;

    state.fire_event(
        project_id,
        "work.activated",
        "work",
        work_id,
        agent_id,
        Some(user_id),
        serde_json::json!({
            "work_id": work_id,
            "project_id": project_id,
            "intent_type": work.intent_type,
        }),
    );

    Ok(Json(work))
}

async fn plan_work(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, work_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PlanWorkResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    // Verify the work item belongs to this project
    let work = state.db.get_work_by_id(work_id).await?;
    if work.project_id != project_id {
        return Err(AppError::NotFound("Work item not found".into()));
    }

    // Ensure Anthropic API key is configured
    let api_key = state.anthropic_api_key.as_deref().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "AI planning is not configured (ANTHROPIC_API_KEY not set)".into(),
        )
    })?;

    // Get project name for context
    let project = state.db.get_project_by_id(project_id).await?;

    let tasks = crate::ai::generate_task_plan(
        api_key,
        &work.title,
        work.description.as_deref().unwrap_or(""),
        &work.success_criteria,
        &project.name,
    )
    .await?;

    Ok(Json(PlanWorkResponse { tasks }))
}

async fn link_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
    Json(req): Json<LinkTaskWork>,
) -> Result<Json<TaskWork>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
        "decide",
    )
    .await?;
    let tw = state.db.link_task_work(work_id, req.task_id).await?;
    refresh_auto_status_works(&state, req.task_id, agent_id).await;
    Ok(Json(tw))
}

async fn unlink_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((work_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
        "decide",
    )
    .await?;
    state.db.unlink_task_work(work_id, task_id).await?;
    refresh_auto_status_works(&state, task_id, agent_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_progress(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
) -> Result<Json<WorkProgress>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    let progress = state.db.get_work_progress(work_id).await?;
    Ok(Json(progress))
}

async fn get_stats(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
) -> Result<Json<WorkStats>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    let stats = state.db.get_work_stats(work_id).await?;
    Ok(Json(stats))
}

async fn list_children(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
) -> Result<Json<Vec<Work>>, AppError> {
    let work = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    let filters = WorkFilters {
        status: None,
        work_type: None,
        parent_work_id: Some(work_id),
        top_level: None,
        limit: Some(100),
        offset: Some(0),
    };
    let children = state.db.list_works(work.project_id, &filters).await?;
    Ok(Json(children))
}

#[derive(Debug, serde::Deserialize)]
struct WorkTasksQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_work_tasks_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
    Query(q): Query<WorkTasksQuery>,
) -> Result<Json<Vec<Task>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    let limit = q.limit.unwrap_or(50).min(100);
    let offset = q.offset.unwrap_or(0);
    let tasks = state.db.list_work_tasks(work_id, limit, offset).await?;
    Ok(Json(tasks))
}

async fn bulk_link_tasks_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
    Json(req): Json<BulkLinkTasks>,
) -> Result<Json<serde_json::Value>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
        "decide",
    )
    .await?;
    let linked = state.db.bulk_link_tasks(work_id, &req.task_ids).await?;
    // Refresh auto-status for all affected tasks
    for task_id in &req.task_ids {
        refresh_auto_status_works(&state, *task_id, agent_id).await;
    }
    Ok(Json(serde_json::json!({ "linked": linked })))
}

// ── Work Comments ──

async fn create_work_comment(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
    Json(req): Json<CreateWorkComment>,
) -> Result<Json<WorkComment>, AppError> {
    let work = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    let comment = state
        .db
        .create_work_comment(work_id, &req, Some(user_id))
        .await?;

    state.fire_event(
        work.project_id,
        "comment.created",
        "work_comment",
        comment.id,
        agent_id,
        Some(user_id),
        serde_json::json!({
            "work_id": work_id,
            "comment_id": comment.id,
            "content": comment.content,
        }),
    );

    Ok(Json(comment))
}

async fn list_work_comments(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(work_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<WorkComment>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_work_by_id(work_id).await?,
    )
    .await?;
    let comments = state.db.list_work_comments(work_id, &pagination).await?;
    Ok(Json(comments))
}

async fn reorder_work_tasks_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, work_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ReorderWorkTasks>,
) -> Result<Json<Vec<Task>>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "decide").await?;

    // Verify the work item belongs to this project
    let existing = state.db.get_work_by_id(work_id).await?;
    if existing.project_id != project_id {
        return Err(AppError::NotFound("Work item not found".into()));
    }

    let tasks = state.db.reorder_work_tasks(work_id, &req.task_ids).await?;
    Ok(Json(tasks))
}

/// For a given task_id, query all linked work items with `auto_status = true`,
/// compute derived status, update if changed, and fire `work.achieved` event
/// if applicable.
pub(crate) async fn refresh_auto_status_works(
    state: &AppState,
    task_id: Uuid,
    agent_id: Option<Uuid>,
) {
    let work_ids = match state.db.list_auto_status_work_ids_for_task(task_id).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to list auto-status work items");
            return;
        }
    };

    for work_id in work_ids {
        let derived = match state.db.compute_auto_status(work_id).await {
            Ok(Some(s)) => s,
            Ok(None) => continue,
            Err(e) => {
                tracing::warn!(work_id = %work_id, error = %e, "Failed to compute auto-status");
                continue;
            }
        };

        let work = match state.db.get_work_by_id(work_id).await {
            Ok(w) => w,
            Err(_) => continue,
        };

        if work.status == derived {
            continue;
        }

        let update = UpdateWork {
            title: None,
            description: None,
            status: Some(derived.clone()),
            work_type: None,
            priority: None,
            parent_work_id: None,
            auto_status: None,
            intent_type: None,
            success_criteria: None,
            metadata: None,
            sort_order: None,
        };

        match state.db.update_work(work_id, &update).await {
            Ok(updated) => {
                if updated.status == "achieved" && work.status != "achieved" {
                    state.fire_event(
                        updated.project_id,
                        "work.achieved",
                        "work",
                        updated.id,
                        agent_id,
                        None,
                        serde_json::json!({"work_id": updated.id, "title": updated.title, "auto": true}),
                    );
                }
            }
            Err(e) => {
                tracing::warn!(work_id = %work_id, error = %e, "Failed to auto-update work status");
            }
        }
    }
}
