use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use std::time::Duration;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::ws_protocol::WsMessage;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{project_id}/playbooks", get(list).post(create))
        .route(
            "/projects/{project_id}/playbooks/{name}",
            get(get_one).put(update).delete(remove),
        )
        .route("/git-strategies", get(git_strategies))
}

async fn proxy_playbook(
    state: &AppState,
    project_id: Uuid,
    operation: &str,
    name: Option<String>,
    content: Option<serde_json::Value>,
) -> Result<serde_json::Value, AppError> {
    let project = state.db.get_project_by_id(project_id).await?;
    let candidates = state.db.list_tenant_agent_ids(project.tenant_id).await?;
    let mut agent_ids = Vec::new();
    for id in candidates {
        if let Ok(agent) = state.db.get_agent_by_id(id).await
            && agent.metadata["playbook_protocol"].as_u64() == Some(1)
        {
            agent_ids.push(id);
        }
    }

    let agent_id = state
        .ws_registry
        .find_connected_agent(&agent_ids)
        .ok_or_else(|| {
            AppError::ServiceUnavailable("No orchestra connected for this project".into())
        })?;

    let request_id = Uuid::new_v4().to_string();
    let rx = state
        .ws_registry
        .register_playbook_request(request_id.clone());

    let sent = state.ws_registry.send_to_agent(
        agent_id,
        WsMessage::PlaybookRequest {
            request_id,
            project_id,
            operation: operation.to_string(),
            name,
            content,
        },
    );

    if !sent {
        return Err(AppError::ServiceUnavailable(
            "Orchestra disconnected".into(),
        ));
    }

    let payload = tokio::time::timeout(Duration::from_secs(10), rx)
        .await
        .map_err(|_| AppError::ServiceUnavailable("Orchestra timed out".into()))?
        .map_err(|_| AppError::ServiceUnavailable("Orchestra disconnected".into()))?;

    if payload.success {
        let mut data = payload.data;
        let normalize = |book: &mut serde_json::Value| {
            if book["name"].is_string() {
                book["id"] = book["name"].clone();
                book["tenant_id"] = serde_json::json!(project.tenant_id);
            }
        };
        if let Some(books) = data.as_array_mut() {
            for book in books {
                normalize(book);
            }
        } else {
            normalize(&mut data);
        }
        Ok(data)
    } else {
        Err(AppError::Internal(
            payload
                .error
                .unwrap_or_else(|| "playbook operation failed".into()),
        ))
    }
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let mut books = diraigent_types::default_playbooks::default_playbooks();
    if let Ok(data) = proxy_playbook(&state, project_id, "list", None, None).await {
        for book in data.as_array().into_iter().flatten() {
            books.retain(|default| default["name"] != book["name"]);
            books.push(book.clone());
        }
    }
    Ok(Json(serde_json::json!(books)))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let name = body["name"].as_str().map(|s| s.to_string());
    let data = proxy_playbook(&state, project_id, "create", name, Some(body)).await?;
    Ok(Json(data))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, name)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    if let Some(book) = diraigent_types::default_playbooks::default_playbooks()
        .into_iter()
        .find(|book| book["name"] == name)
    {
        return Ok(Json(book));
    }
    let data = proxy_playbook(&state, project_id, "get", Some(name), None).await?;
    Ok(Json(data))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, name)): Path<(Uuid, String)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let data = if diraigent_types::default_playbooks::default_playbooks()
        .iter()
        .any(|book| book["name"] == name)
    {
        proxy_playbook(
            &state,
            project_id,
            "create",
            Some(format!("{name}-custom")),
            Some(body),
        )
        .await?
    } else {
        proxy_playbook(&state, project_id, "update", Some(name), Some(body)).await?
    };
    Ok(Json(data))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, name)): Path<(Uuid, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    if diraigent_types::default_playbooks::default_playbooks()
        .iter()
        .any(|book| book["name"] == name)
    {
        return Err(AppError::Validation(
            "Bundled default playbooks cannot be deleted".into(),
        ));
    }
    let data = proxy_playbook(&state, project_id, "delete", Some(name), None).await?;
    Ok(Json(data))
}

/// Static catalog of immutable git strategies.
async fn git_strategies() -> Json<serde_json::Value> {
    Json(serde_json::json!([
        {
            "id": "merge",
            "name": "Merge",
            "description": "Branch from target (default branch unless overridden), merge back when done. Standard autonomous workflow.",
            "fields": { "git_target_branch": "string (optional, defaults to project default branch)" },
        },
        {
            "id": "branch_only",
            "name": "Branch Only (No Merge)",
            "description": "Branch from default, push branch to origin. No automatic merge. For PR-based workflows.",
        },
        {
            "id": "feature_branch",
            "name": "Feature Branch (Goal-based)",
            "description": "Tasks branch from and merge into a goal branch. The goal branch merges to default when the goal is completed.",
        },
        {
            "id": "no_git",
            "name": "No Git",
            "description": "Plain directory, no git operations. For non-code tasks.",
        },
    ]))
}
