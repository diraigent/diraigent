use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::ws_protocol::WsMessage;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/git/branches", get(list_branches))
        .route(
            "/{project_id}/git/task-branch/{task_id}",
            get(task_branch_status),
        )
        .route("/{project_id}/git/push", post(push_branch))
        .route("/{project_id}/git/main-status", get(main_status))
        .route("/{project_id}/git/push-main", post(push_main))
        .route(
            "/{project_id}/git/resolve-and-push-main",
            post(resolve_and_push_main),
        )
        .route("/{project_id}/git/revert-task/{task_id}", post(revert_task))
        .route(
            "/{project_id}/git/resolve-task-branch/{task_id}",
            post(resolve_task_branch),
        )
        .route("/{project_id}/git/release", post(release))
}

// ── Types (unchanged HTTP contract) ──

#[derive(Debug, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub commit: String,
    pub is_pushed: bool,
    pub ahead_remote: i32,
    pub behind_remote: i32,
    pub task_id_prefix: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BranchListResponse {
    pub current_branch: String,
    pub branches: Vec<BranchInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskBranchStatus {
    pub branch: String,
    pub exists: bool,
    pub is_pushed: bool,
    pub ahead_remote: i32,
    pub behind_remote: i32,
    pub last_commit: Option<String>,
    pub last_commit_message: Option<String>,
    #[serde(default)]
    pub behind_default: i32,
    #[serde(default)]
    pub has_conflict: bool,
}

#[derive(Debug, Deserialize)]
pub struct PushRequest {
    pub branch: String,
    pub remote: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MainPushStatus {
    pub ahead: i32,
    pub behind: i32,
    pub last_commit: Option<String>,
    pub last_commit_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseRequest {
    /// Source branch to squash-merge from (default: "dev")
    pub source_branch: Option<String>,
    /// Commit message (auto-generated from git log if not provided)
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct BranchFilter {
    pub prefix: Option<String>,
}

// ── Handlers ──

async fn list_branches(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<BranchFilter>,
) -> Result<Json<BranchListResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "list_branches",
            prefix: filter.prefix.or(Some("agent/".into())),
            ..Default::default()
        },
    )
    .await?;

    let result: BranchListResponse = serde_json::from_value(data)
        .map_err(|e| AppError::Internal(format!("failed to parse git response: {e}")))?;

    Ok(Json(result))
}

async fn task_branch_status(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<TaskBranchStatus>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "task_branch_status",
            task_id: Some(task_id.to_string()),
            ..Default::default()
        },
    )
    .await?;

    let result: TaskBranchStatus = serde_json::from_value(data)
        .map_err(|e| AppError::Internal(format!("failed to parse git response: {e}")))?;

    Ok(Json(result))
}

async fn push_branch(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<PushRequest>,
) -> Result<Json<PushResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    if !req.branch.starts_with("agent/") {
        return Err(AppError::Validation(
            "Only agent/* branches can be pushed from the API".into(),
        ));
    }

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "push",
            branch: Some(req.branch.clone()),
            remote: req.remote.clone(),
            ..Default::default()
        },
    )
    .await?;

    let message = data["message"]
        .as_str()
        .unwrap_or("Push completed")
        .to_string();

    Ok(Json(PushResponse {
        success: true,
        message,
    }))
}

async fn main_status(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<MainPushStatus>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "main_status",
            ..Default::default()
        },
    )
    .await?;

    let result: MainPushStatus = serde_json::from_value(data)
        .map_err(|e| AppError::Internal(format!("failed to parse git response: {e}")))?;

    Ok(Json(result))
}

async fn push_main(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<PushResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "push_main",
            ..Default::default()
        },
    )
    .await?;

    let message = data["message"]
        .as_str()
        .unwrap_or("Push completed")
        .to_string();

    Ok(Json(PushResponse {
        success: true,
        message,
    }))
}

async fn resolve_and_push_main(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<PushResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "resolve_and_push_main",
            ..Default::default()
        },
    )
    .await?;

    let message = data["message"]
        .as_str()
        .unwrap_or("Resolved and pushed")
        .to_string();

    Ok(Json(PushResponse {
        success: true,
        message,
    }))
}

async fn revert_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PushResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "revert_task",
            task_id: Some(task_id.to_string()),
            ..Default::default()
        },
    )
    .await?;

    // Mark the task as reverted
    sqlx::query("UPDATE diraigent.task SET reverted_at = NOW() WHERE id = $1")
        .bind(task_id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to mark task as reverted: {e}")))?;

    let message = data["message"]
        .as_str()
        .unwrap_or("Revert completed")
        .to_string();

    Ok(Json(PushResponse {
        success: true,
        message,
    }))
}

async fn resolve_task_branch(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PushResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "resolve_task_branch",
            task_id: Some(task_id.to_string()),
            ..Default::default()
        },
    )
    .await?;

    let message = data["message"]
        .as_str()
        .unwrap_or("Resolve completed")
        .to_string();

    Ok(Json(PushResponse {
        success: true,
        message,
    }))
}

async fn release(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ReleaseRequest>,
) -> Result<Json<ReleaseResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;

    let data = git_ws_request_with_timeout(
        &state,
        project_id,
        GitWsParams {
            query_type: "release",
            branch: req.source_branch,
            path: req.message,
            ..Default::default()
        },
        60, // Longer timeout: squash merge + push to multiple remotes
    )
    .await?;

    let message = data["message"]
        .as_str()
        .unwrap_or("Release completed")
        .to_string();

    Ok(Json(ReleaseResponse {
        success: true,
        message,
    }))
}

// ── WebSocket request-reply helper ──

#[derive(Default)]
pub(crate) struct GitWsParams<'a> {
    pub query_type: &'a str,
    pub prefix: Option<String>,
    pub task_id: Option<String>,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub path: Option<String>,
    pub git_ref: Option<String>,
}

pub(crate) async fn git_ws_request(
    state: &AppState,
    project_id: Uuid,
    params: GitWsParams<'_>,
) -> Result<serde_json::Value, AppError> {
    git_ws_request_with_timeout(state, project_id, params, 10).await
}

async fn git_ws_request_with_timeout(
    state: &AppState,
    project_id: Uuid,
    params: GitWsParams<'_>,
    timeout_secs: u64,
) -> Result<serde_json::Value, AppError> {
    let project = state.db.get_project_by_id(project_id).await?;

    // git_mode=none: return empty/default responses without hitting the orchestra
    if project.git_mode == "none" {
        return Ok(match params.query_type {
            "list_branches" => serde_json::json!({
                "current_branch": "",
                "branches": []
            }),
            "task_branch_status" => serde_json::json!({
                "branch": "",
                "exists": false,
                "is_pushed": false,
                "ahead_remote": 0,
                "behind_remote": 0,
                "last_commit": null,
                "last_commit_message": null,
                "behind_default": 0,
                "has_conflict": false
            }),
            "main_status" => serde_json::json!({
                "ahead": 0,
                "behind": 0,
                "last_commit": null,
                "last_commit_message": null
            }),
            "source_tree" => serde_json::json!({ "entries": [] }),
            "source_blob" => {
                serde_json::json!({ "not_found": true, "error": "Git disabled for this project" })
            }
            "push_main" | "resolve_and_push_main" | "push" | "release" => {
                return Err(AppError::Validation(
                    "Git operations are disabled for this project".into(),
                ));
            }
            _ => serde_json::json!({}),
        });
    }

    // Resolve git_ref to the project's default_branch when not provided.
    // This is primarily used by source_tree/source_blob endpoints so the API
    // is the single source-of-truth for the default ref.
    let git_ref = params
        .git_ref
        .or_else(|| Some(project.default_branch.clone()));

    let agent_ids = state
        .db
        .list_tenant_agent_ids(project.tenant_id)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to find agents: {e}")))?;

    let agent_id =
        state
            .ws_registry
            .find_connected_agent(&agent_ids)
            .ok_or(AppError::ServiceUnavailable(
                "No orchestra agent connected for this project".into(),
            ))?;

    let request_id = Uuid::now_v7().to_string();
    let rx = state.ws_registry.register_git_request(request_id.clone());

    let msg = WsMessage::GitRequest {
        request_id: request_id.clone(),
        project_id,
        query_type: params.query_type.into(),
        prefix: params.prefix,
        task_id: params.task_id,
        branch: params.branch,
        remote: params.remote,
        path: params.path,
        git_ref,
    };

    if !state.ws_registry.send_to_agent(agent_id, msg) {
        return Err(AppError::ServiceUnavailable(
            "Failed to send to orchestra".into(),
        ));
    }

    let response = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx)
        .await
        .map_err(|_| {
            AppError::ServiceUnavailable(
                "Git operation timed out (orchestra may be unavailable)".into(),
            )
        })?
        .map_err(|_| AppError::Internal("Orchestra connection dropped".into()))?;

    if !response.success {
        return Err(AppError::Internal(format!(
            "Git operation failed: {}",
            response.error.unwrap_or("unknown".into())
        )));
    }

    Ok(response.data)
}
