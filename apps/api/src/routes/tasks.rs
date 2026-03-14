use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{
    OptionalAgentId, authorities_for_claim, ensure_any_authority_on, ensure_authority_on,
    ensure_member, require_authority, require_membership,
};
use crate::error::AppError;
use crate::models::*;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Project-scoped
        .route("/{project_id}/tasks", post(create_task).get(list_tasks))
        .route("/{project_id}/tasks/ready", get(list_ready_tasks))
        .route("/{project_id}/tasks/blocked", get(list_blocked_task_ids))
        .route("/{project_id}/tasks/flagged", get(list_flagged_task_ids))
        .route(
            "/{project_id}/tasks/with-blockers",
            get(list_tasks_with_blocker_updates),
        )
        .route(
            "/{project_id}/tasks/work-linked",
            get(list_work_linked_task_ids),
        )
        .route(
            "/{project_id}/tasks/bulk/transition",
            post(bulk_transition_tasks),
        )
        .route(
            "/{project_id}/tasks/bulk/delegate",
            post(bulk_delegate_tasks),
        )
        .route("/{project_id}/tasks/bulk/delete", post(bulk_delete_tasks))
        // Cross-project task operations
        .route(
            "/tasks/{task_id}",
            get(get_task).put(update_task).delete(delete_task),
        )
        .route("/tasks/{task_id}/transition", post(transition_task))
        .route("/tasks/{task_id}/claim", post(claim_task))
        .route("/tasks/{task_id}/release", post(release_task))
        .route("/tasks/{task_id}/delegate", post(delegate_task_handler))
        .route(
            "/tasks/{task_id}/dependencies",
            get(list_dependencies).post(add_dependency),
        )
        .route(
            "/tasks/{task_id}/dependencies/{dep_id}",
            delete(remove_dependency),
        )
        .route(
            "/tasks/{task_id}/updates",
            get(list_task_updates).post(create_task_update),
        )
        .route(
            "/tasks/{task_id}/comments",
            get(list_task_comments).post(create_task_comment),
        )
        .route("/tasks/{task_id}/work", get(list_task_works))
        .route("/tasks/{task_id}/children", get(list_task_children))
        .route("/tasks/{task_id}/related", get(get_related_items))
        .route("/tasks/{task_id}/cost", post(record_task_cost))
}

async fn create_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateTask>,
) -> Result<Json<Task>, AppError> {
    let pkg = state.pkg_cache.get_for_project(project_id).await?;
    validation::validate_create_task(&req, pkg.as_ref())?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "create").await?;

    // If playbook_id provided, verify it exists
    if let Some(playbook_id) = req.playbook_id {
        let _ = state.db.get_playbook_by_id(playbook_id).await?;
    }

    // If work_id provided, verify the work item exists before creating the task
    if let Some(work_id) = req.work_id {
        let _ = state.db.get_work_by_id(work_id).await?;
    }

    // If parent_id provided, verify the parent task exists and belongs to the same project
    if let Some(parent_id) = req.parent_id {
        let parent = state.db.get_task_by_id(parent_id).await?;
        if parent.project_id != project_id {
            return Err(AppError::UnprocessableEntity(
                "parent task must belong to the same project".into(),
            ));
        }
    }

    let task = state.db.create_task(project_id, &req, user_id).await?;

    // If task was auto-transitioned to ready (has playbook with initial_state=ready), fire event
    if task.playbook_id.is_some() && task.state == "ready" {
        state.fire_event(
            task.project_id,
            "task.transitioned",
            "task",
            task.id,
            agent_id,
            Some(user_id),
            serde_json::json!({
                "task_id": task.id,
                "title": task.title,
                "from": "backlog",
                "to": "ready",
                "playbook_id": task.playbook_id,
                "playbook_step": task.playbook_step,
            }),
        );
    }

    // If work_id provided, link the new task to the work item atomically
    if let Some(work_id) = req.work_id {
        state.db.link_task_work(work_id, task.id).await?;
        super::work::refresh_auto_status_works(&state, task.id, agent_id).await;
    }

    // Inherit work links from the creating agent's active tasks (subtask work inheritance)
    if let Some(aid) = agent_id
        && let Ok(inherited_work_ids) = state
            .db
            .get_agent_inherited_work_ids(aid, project_id, task.id)
            .await
    {
        for work_id in inherited_work_ids {
            // Skip if already linked via explicit work_id
            if req.work_id == Some(work_id) {
                continue;
            }
            if let Err(e) = state.db.link_task_work(work_id, task.id).await {
                tracing::warn!(
                    task_id = %task.id,
                    work_id = %work_id,
                    error = %e,
                    "Failed to inherit work link from agent's active task"
                );
            }
        }
        // Refresh auto-status for any newly inherited work items
        super::work::refresh_auto_status_works(&state, task.id, agent_id).await;
    }

    Ok(Json(task))
}

async fn list_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<TaskFilters>,
) -> Result<Json<PaginatedResponse<Task>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_tasks(project_id, &filters),
        state.db.count_tasks(project_id, &filters),
    )
    .await
}

async fn list_ready_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Task>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let limit = pagination.limit.unwrap_or(50).min(100);
    let offset = pagination.offset.unwrap_or(0);
    let tasks = state.db.list_ready_tasks(project_id, limit, offset).await?;
    Ok(Json(tasks))
}

async fn list_blocked_task_ids(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let ids = state.db.list_blocked_task_ids(project_id).await?;
    Ok(Json(ids))
}

async fn list_flagged_task_ids(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let ids = state.db.list_flagged_task_ids(project_id).await?;
    Ok(Json(ids))
}

async fn list_work_linked_task_ids(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let ids = state.db.list_work_linked_task_ids(project_id).await?;
    Ok(Json(ids))
}

async fn list_tasks_with_blocker_updates(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Task>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let tasks = state.db.list_tasks_with_blocker_updates(project_id).await?;
    Ok(Json(tasks))
}

/// Return all work IDs linked to a task.
async fn list_task_works(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, AppError> {
    let task = state.db.get_task_by_id(task_id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, task.project_id).await?;
    let ids = state.db.get_work_ids_for_task(task_id).await?;
    Ok(Json(ids))
}

/// Return direct child tasks of a given parent task.
async fn list_task_children(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Vec<Task>>, AppError> {
    let task = state.db.get_task_by_id(task_id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, task.project_id).await?;
    let children = state.db.list_task_children(task_id).await?;
    Ok(Json(children))
}

/// Query parameters for the related-items endpoint.
#[derive(Debug, Deserialize)]
struct RelatedItemsQuery {
    /// Maximum number of items to return per entity type (default 5, max 20).
    limit: Option<i64>,
}

/// `GET /tasks/{task_id}/related`
///
/// Return knowledge entries, decisions, and observations related to the given
/// task, ranked by relevance. Uses keyword and file-path matching against the
/// task's title, spec, acceptance criteria, and context.files.
async fn get_related_items(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Query(params): Query<RelatedItemsQuery>,
) -> Result<Json<RelatedItems>, AppError> {
    let task = state.db.get_task_by_id(task_id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, task.project_id).await?;

    let limit = params.limit.unwrap_or(5).clamp(1, 20) as usize;

    let mut related =
        crate::repository::find_related_for_task(&state.pool, task.project_id, &task).await?;

    // Cap each entity-type list to the requested limit.
    related.knowledge.truncate(limit);
    related.decisions.truncate(limit);
    related.observations.truncate(limit);

    Ok(Json(related))
}

async fn get_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskWithDecision>, AppError> {
    let task = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
    )
    .await?;

    // Fetch originating decision summary if linked.
    let decision = if let Some(decision_id) = task.decision_id {
        state
            .db
            .get_decision_by_id(decision_id)
            .await
            .ok()
            .map(|d| {
                let rationale_excerpt = d.rationale.as_deref().map(|r| {
                    if r.len() > 300 {
                        r[..300].to_string()
                    } else {
                        r.to_string()
                    }
                });
                DecisionSummary {
                    id: d.id,
                    title: d.title,
                    status: d.status,
                    rationale_excerpt,
                }
            })
    } else {
        None
    };

    Ok(Json(TaskWithDecision { task, decision }))
}

async fn update_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<UpdateTask>,
) -> Result<Json<Task>, AppError> {
    let existing = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "create",
    )
    .await?;
    let pkg = state.pkg_cache.get_for_project(existing.project_id).await?;
    validation::validate_update_task(&req, pkg.as_ref())?;
    let task = state.db.update_task(task_id, &req).await?;
    Ok(Json(task))
}

async fn delete_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<(), AppError> {
    let task = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "manage",
    )
    .await?;
    state.db.delete_task(task_id).await?;

    state.fire_event(
        task.project_id,
        "task.deleted",
        "task",
        task.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"task_id": task.id, "title": task.title}),
    );

    Ok(())
}

async fn transition_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<TransitionTask>,
) -> Result<Json<Task>, AppError> {
    // Capture before-state for audit; also verifies execute authority.
    let before = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "execute",
    )
    .await?;
    let old_state = before.state.clone();

    // Completing a "review" step additionally requires review authority.
    // (execute check already passed above)
    if req.state == "done" && old_state == "review" {
        require_authority(
            state.db.as_ref(),
            agent_id,
            user_id,
            before.project_id,
            "review",
        )
        .await?;
    }

    let task = state
        .db
        .transition_task(task_id, &req.state, req.playbook_step)
        .await?;

    // Auto-release file locks when task completes, is cancelled, or advances
    // to the next pipeline step (step → ready advancement).
    if matches!(task.state.as_str(), "done" | "cancelled")
        || (task.state == "ready" && !is_lifecycle_state(&old_state))
    {
        let _ = state.db.release_file_locks_for_task(task_id).await;
    }

    crate::metrics::record_task_transition(&old_state, &task.state);

    state.fire_event(
        task.project_id,
        "task.transitioned",
        "task",
        task.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"task_id": task.id, "title": task.title, "from": old_state, "to": task.state}),
    );

    // Notify SSE review stream when a task enters or leaves human_review.
    if task.state == "human_review" || old_state == "human_review" {
        let kind = if task.state == "human_review" {
            "entered"
        } else {
            "left"
        };
        let _ = state.review_tx.send(crate::ReviewSseEvent {
            kind: kind.to_string(),
            project_id: task.project_id,
            task_id: task.id,
            title: task.title.clone(),
        });
    }

    // Sync linked report status when a task transitions to done or cancelled.
    sync_report_status(&state, task_id, &task.state).await;

    // Refresh auto-status work items linked to this task.
    crate::routes::work::refresh_auto_status_works(&state, task_id, agent_id).await;

    Ok(Json(task))
}

async fn claim_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<ClaimTask>,
) -> Result<Json<Task>, AppError> {
    // Fetch task, then check authority for the playbook step being entered.
    // "review" step accepts either execute or review authority.
    let before = state.db.get_task_by_id(task_id).await?;
    let step_name = state.db.resolve_claim_step_name(&before).await?;
    let allowed = authorities_for_claim(&step_name);
    ensure_any_authority_on(state.db.as_ref(), agent_id, user_id, before, &allowed).await?;

    let task = state.db.claim_task(task_id, req.agent_id).await?;

    state.fire_event(
        task.project_id,
        "task.claimed", // event name kept for backward compat
        "task",
        task.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"task_id": task.id, "title": task.title, "agent_id": req.agent_id}),
    );

    Ok(Json(task))
}

async fn release_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<Json<Task>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "execute",
    )
    .await?;
    let task = state.db.release_task(task_id).await?;

    state.fire_event(
        task.project_id,
        "task.released",
        "task",
        task.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"task_id": task.id, "title": task.title}),
    );

    Ok(Json(task))
}

async fn delegate_task_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<DelegateTask>,
) -> Result<Json<Task>, AppError> {
    let before = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "delegate",
    )
    .await?;

    let task = state
        .db
        .delegate_task(task_id, user_id, req.agent_id, req.role_id)
        .await?;

    state.fire_event(
        task.project_id,
        "task.delegated",
        "task",
        task.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"task_id": task.id, "title": task.title, "to_agent_id": req.agent_id, "from_agent_id": before.assigned_agent_id}),
    );

    Ok(Json(task))
}

async fn list_dependencies(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskDependencies>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
    )
    .await?;
    let deps = state.db.list_dependencies(task_id).await?;
    Ok(Json(deps))
}

async fn add_dependency(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<AddDependency>,
) -> Result<Json<TaskDependency>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "create",
    )
    .await?;
    let dep = state.db.add_dependency(task_id, req.depends_on).await?;
    Ok(Json(dep))
}

async fn remove_dependency(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((task_id, dep_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "create",
    )
    .await?;
    state.db.remove_dependency(task_id, dep_id).await?;
    Ok(())
}

async fn list_task_updates(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<TaskUpdate>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
    )
    .await?;
    let updates = state.db.list_task_updates(task_id, &pagination).await?;
    Ok(Json(updates))
}

async fn create_task_update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(mut req): Json<CreateTaskUpdate>,
) -> Result<Json<TaskUpdate>, AppError> {
    let task = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "execute",
    )
    .await?;
    // Use header-extracted agent_id when not provided in the request body
    if req.agent_id.is_none() {
        req.agent_id = agent_id;
    }
    let update = state
        .db
        .create_task_update(task_id, &req, Some(user_id))
        .await?;

    state.fire_event(
        task.project_id,
        "update.created",
        "task_update",
        update.id,
        agent_id,
        Some(user_id),
        serde_json::json!({
            "task_id": task_id,
            "update_id": update.id,
            "kind": update.kind,
            "content": update.content,
        }),
    );

    Ok(Json(update))
}

async fn list_task_comments(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<TaskComment>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
    )
    .await?;
    let comments = state.db.list_task_comments(task_id, &pagination).await?;
    Ok(Json(comments))
}

async fn create_task_comment(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(mut req): Json<CreateTaskComment>,
) -> Result<Json<TaskComment>, AppError> {
    let task = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_task_by_id(task_id).await?,
        "execute",
    )
    .await?;
    // Use header-extracted agent_id when not provided in the request body
    if req.agent_id.is_none() {
        req.agent_id = agent_id;
    }
    let comment = state
        .db
        .create_task_comment(task_id, &req, Some(user_id))
        .await?;

    state.fire_event(
        task.project_id,
        "comment.created",
        "task_comment",
        comment.id,
        agent_id,
        Some(user_id),
        serde_json::json!({
            "task_id": task_id,
            "comment_id": comment.id,
            "content": comment.content,
        }),
    );

    Ok(Json(comment))
}

// ── Bulk Actions ──

/// Describes the per-task operation for [`bulk_operate`].
enum BulkAction {
    Transition {
        target_state: String,
        playbook_step: Option<i32>,
    },
    Delegate {
        delegated_by: Uuid,
        target_agent_id: Uuid,
        role_id: Option<Uuid>,
    },
    Delete,
}

/// Run a bulk operation over tasks, handling the get_task + project_id guard +
/// succeeded/failed collection boilerplate.
///
/// For each task_id: fetches the task, verifies it belongs to `project_id`,
/// then executes the [`BulkAction`]. Fires the appropriate event on success
/// and collects results into `(Vec<Uuid>, Vec<BulkFailure>)`.
async fn bulk_operate(
    state: &AppState,
    project_id: Uuid,
    task_ids: &[Uuid],
    action: &BulkAction,
    agent_id: Option<Uuid>,
    user_id: Uuid,
) -> (Vec<Uuid>, Vec<BulkFailure>) {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for task_id in task_ids {
        match state.db.get_task_by_id(*task_id).await {
            Ok(task) if task.project_id == project_id => {
                let result = match action {
                    BulkAction::Transition {
                        target_state,
                        playbook_step,
                    } => 'transition: {
                        let old_state = task.state.clone();
                        // Completing a "review" step additionally requires
                        // review authority (mirrors single-task transition_task).
                        if target_state == "done"
                            && old_state == "review"
                            && let Err(e) = require_authority(
                                state.db.as_ref(),
                                agent_id,
                                user_id,
                                project_id,
                                "review",
                            )
                            .await
                        {
                            break 'transition Err(e.to_string());
                        }
                        match state
                            .db
                            .transition_task(*task_id, target_state, *playbook_step)
                            .await
                        {
                            Ok(new_task) => {
                                if matches!(new_task.state.as_str(), "done" | "cancelled")
                                    || (new_task.state == "ready"
                                        && !is_lifecycle_state(&old_state))
                                {
                                    let _ = state.db.release_file_locks_for_task(*task_id).await;
                                }
                                crate::metrics::record_task_transition(&old_state, &new_task.state);
                                state.fire_event(
                                    project_id,
                                    "task.transitioned",
                                    "task",
                                    *task_id,
                                    agent_id,
                                    Some(user_id),
                                    serde_json::json!({"task_id": task_id, "title": new_task.title, "from": old_state, "to": new_task.state}),
                                );
                                crate::routes::work::refresh_auto_status_works(
                                    state, *task_id, agent_id,
                                )
                                .await;
                                Ok(())
                            }
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    BulkAction::Delegate {
                        delegated_by,
                        target_agent_id,
                        role_id,
                    } => {
                        match state
                            .db
                            .delegate_task(*task_id, *delegated_by, *target_agent_id, *role_id)
                            .await
                        {
                            Ok(new_task) => {
                                state.fire_event(
                                    project_id,
                                    "task.delegated",
                                    "task",
                                    *task_id,
                                    agent_id,
                                    Some(user_id),
                                    serde_json::json!({"task_id": task_id, "title": new_task.title, "to_agent_id": target_agent_id, "from_agent_id": task.assigned_agent_id}),
                                );
                                Ok(())
                            }
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    BulkAction::Delete => match state.db.delete_task(*task_id).await {
                        Ok(()) => {
                            state.fire_event(
                                project_id,
                                "task.deleted",
                                "task",
                                *task_id,
                                agent_id,
                                Some(user_id),
                                serde_json::json!({"task_id": task_id, "title": task.title}),
                            );
                            Ok(())
                        }
                        Err(e) => Err(e.to_string()),
                    },
                };
                match result {
                    Ok(()) => succeeded.push(*task_id),
                    Err(e) => failed.push(BulkFailure {
                        task_id: *task_id,
                        error: e,
                    }),
                }
            }
            Ok(_) => failed.push(BulkFailure {
                task_id: *task_id,
                error: "Task does not belong to this project".into(),
            }),
            Err(e) => failed.push(BulkFailure {
                task_id: *task_id,
                error: e.to_string(),
            }),
        }
    }

    (succeeded, failed)
}

async fn bulk_transition_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<BulkTransition>,
) -> Result<(StatusCode, Json<BulkResult>), AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;
    let action = BulkAction::Transition {
        target_state: req.state,
        playbook_step: req.playbook_step,
    };
    let (succeeded, failed) = bulk_operate(
        &state,
        project_id,
        &req.task_ids,
        &action,
        agent_id,
        user_id,
    )
    .await;
    let status = bulk_status_code(&succeeded, &failed);
    Ok((status, Json(BulkResult { succeeded, failed })))
}

async fn bulk_delegate_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<BulkDelegate>,
) -> Result<(StatusCode, Json<BulkResult>), AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "delegate").await?;
    let action = BulkAction::Delegate {
        delegated_by: user_id,
        target_agent_id: req.agent_id,
        role_id: req.role_id,
    };
    let (succeeded, failed) = bulk_operate(
        &state,
        project_id,
        &req.task_ids,
        &action,
        agent_id,
        user_id,
    )
    .await;
    let status = bulk_status_code(&succeeded, &failed);
    Ok((status, Json(BulkResult { succeeded, failed })))
}

async fn bulk_delete_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<BulkDelete>,
) -> Result<(StatusCode, Json<BulkResult>), AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let (succeeded, failed) = bulk_operate(
        &state,
        project_id,
        &req.task_ids,
        &BulkAction::Delete,
        agent_id,
        user_id,
    )
    .await;
    let status = bulk_status_code(&succeeded, &failed);
    Ok((status, Json(BulkResult { succeeded, failed })))
}

/// Determine the HTTP status code for a bulk operation result.
/// - 200 OK: all tasks succeeded (or the batch was empty)
/// - 207 Multi-Status: some succeeded, some failed
/// - 400 Bad Request: all tasks failed
fn bulk_status_code(succeeded: &[Uuid], failed: &[BulkFailure]) -> StatusCode {
    if failed.is_empty() {
        StatusCode::OK
    } else if succeeded.is_empty() {
        StatusCode::BAD_REQUEST
    } else {
        // 207 Multi-Status: partial success
        StatusCode::MULTI_STATUS
    }
}

/// `POST /tasks/{task_id}/cost`
///
/// Accumulate LLM token usage and cost for a task. Called by the orchestra
/// after each Claude Code step completes. Values are added to the existing
/// totals on the task row.
async fn record_task_cost(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(task_id): Path<Uuid>,
    Json(req): Json<TaskCostUpdate>,
) -> Result<Json<Task>, AppError> {
    // Resolve project_id for the membership check
    let task = state.db.get_task_by_id(task_id).await?;
    ensure_member(state.db.as_ref(), agent_id, user_id, task).await?;

    let updated = state
        .db
        .update_task_cost(task_id, req.input_tokens, req.output_tokens, req.cost_usd)
        .await?;

    Ok(Json(updated))
}

/// When a task transitions to `done` or `cancelled`, check if there is a linked
/// report (i.e. a report whose `task_id` matches this task). If so, update the
/// report status accordingly — `completed` for done, `failed` for cancelled.
/// This is a best-effort fallback; agents can also call the explicit
/// `POST /{project_id}/reports/{id}/complete` endpoint.
async fn sync_report_status(state: &AppState, task_id: Uuid, new_state: &str) {
    let report_status = match new_state {
        "done" => "completed",
        "cancelled" => "failed",
        _ => return,
    };

    match state.db.get_report_by_task_id(task_id).await {
        Ok(Some(report)) if report.status == "in_progress" => {
            let update_req = UpdateReport {
                title: None,
                status: Some(report_status.to_string()),
                result: None,
                task_id: None,
                metadata: None,
            };
            if let Err(e) = state.db.update_report(report.id, &update_req).await {
                tracing::warn!(
                    report_id = %report.id,
                    task_id = %task_id,
                    error = %e,
                    "Failed to sync report status on task transition"
                );
            } else {
                state.fire_event(
                    report.project_id,
                    "report.status_synced",
                    "report",
                    report.id,
                    None,
                    None,
                    serde_json::json!({
                        "report_id": report.id,
                        "task_id": task_id,
                        "status": report_status,
                    }),
                );
            }
        }
        Ok(_) => {} // No linked report or already completed/failed
        Err(e) => {
            tracing::warn!(
                task_id = %task_id,
                error = %e,
                "Failed to check for linked report on task transition"
            );
        }
    }
}
