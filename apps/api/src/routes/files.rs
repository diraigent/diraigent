use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::path::Path as StdPath;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/{project_id}/claude-md",
        get(get_claude_md).put(put_claude_md),
    )
}

#[derive(Debug, Serialize)]
pub struct ClaudeMdResponse {
    pub content: String,
    pub exists: bool,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeMdUpdate {
    pub content: String,
}

async fn get_claude_md(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ClaudeMdResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let repo_root = match state.repo_root.as_ref() {
        Some(r) => r,
        None => {
            return Ok(Json(ClaudeMdResponse {
                content: String::new(),
                exists: false,
            }));
        }
    };

    let claude_md_path = repo_root.join("CLAUDE.md");
    match tokio::fs::read_to_string(&claude_md_path).await {
        Ok(content) => Ok(Json(ClaudeMdResponse {
            content,
            exists: true,
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Json(ClaudeMdResponse {
            content: String::new(),
            exists: false,
        })),
        Err(e) => Err(AppError::Internal(format!("Failed to read CLAUDE.md: {e}"))),
    }
}

async fn put_claude_md(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ClaudeMdUpdate>,
) -> Result<Json<ClaudeMdResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let repo_root = state.repo_root.as_ref().ok_or_else(|| {
        AppError::ServiceUnavailable(
            "REPO_ROOT not configured — file operations unavailable".into(),
        )
    })?;

    let claude_md_path = repo_root.join("CLAUDE.md");

    // Validate path stays within repo root (prevent traversal)
    let canonical_root = StdPath::new(repo_root)
        .canonicalize()
        .map_err(|e| AppError::Internal(format!("Cannot resolve repo root: {e}")))?;

    // For new files, canonicalize the parent
    let parent = claude_md_path
        .parent()
        .ok_or_else(|| AppError::Internal("Invalid path".into()))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| AppError::Internal(format!("Cannot resolve path: {e}")))?;

    if !canonical_parent.starts_with(&canonical_root) {
        return Err(AppError::Validation("Path traversal not allowed".into()));
    }

    tokio::fs::write(&claude_md_path, &req.content)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write CLAUDE.md: {e}")))?;

    Ok(Json(ClaudeMdResponse {
        content: req.content,
        exists: true,
    }))
}
