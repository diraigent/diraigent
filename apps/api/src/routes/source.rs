use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;

use super::git::{GitWsParams, git_ws_request};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/source/tree", get(source_tree))
        .route("/{project_id}/source/blob", get(source_blob))
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TreeQuery {
    #[serde(default)]
    pub path: String,
    /// Git ref (branch/tag/commit). When omitted, the orchestra resolves to
    /// the project's `default_branch` via `WorktreeManager::default_branch()`.
    pub r#ref: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlobQuery {
    #[serde(default)]
    pub path: String,
    /// Git ref (branch/tag/commit). When omitted, the orchestra resolves to
    /// the project's `default_branch`.
    pub r#ref: Option<String>,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreeResponse {
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlobResponse {
    pub content: String,
    pub encoding: String,
    pub size: usize,
}

// ── Custom 413 response ───────────────────────────────────────────────────────

struct PayloadTooLarge(String);

impl IntoResponse for PayloadTooLarge {
    fn into_response(self) -> Response {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "error": self.0,
                "errorCode": "PAYLOAD_TOO_LARGE",
            })),
        )
            .into_response()
    }
}

// ── Path validation ───────────────────────────────────────────────────────────

/// Validate that the requested path doesn't escape the repo root.
/// Returns the canonical string form of the path (relative to repo root).
fn validate_path(path: &str) -> Result<String, AppError> {
    use std::path::{Component, Path as StdPath};

    // Reject obvious traversal attempts and absolute paths.
    let p = StdPath::new(path);
    for component in p.components() {
        match component {
            Component::ParentDir => {
                return Err(AppError::Validation("Path traversal not allowed".into()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::Validation("Absolute paths not allowed".into()));
            }
            _ => {}
        }
    }

    // Normalise by collecting non-CurDir components.
    let clean: std::path::PathBuf = p
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();

    Ok(clean.to_string_lossy().into_owned())
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn source_tree(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(q): Query<TreeQuery>,
) -> Result<Json<TreeResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let clean_path = validate_path(&q.path)?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "source_tree",
            path: Some(clean_path),
            git_ref: q.r#ref,
            ..Default::default()
        },
    )
    .await?;

    if let Some(true) = data.get("not_found").and_then(|v| v.as_bool()) {
        let msg = data["error"].as_str().unwrap_or("Not found").to_string();
        return Err(AppError::NotFound(msg));
    }

    let result: TreeResponse = serde_json::from_value(data)
        .map_err(|e| AppError::Internal(format!("failed to parse source tree response: {e}")))?;

    Ok(Json(result))
}

async fn source_blob(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(q): Query<BlobQuery>,
) -> Result<Response, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let clean_path = validate_path(&q.path)?;

    let data = git_ws_request(
        &state,
        project_id,
        GitWsParams {
            query_type: "source_blob",
            path: Some(clean_path),
            git_ref: q.r#ref,
            ..Default::default()
        },
    )
    .await?;

    if let Some(true) = data.get("not_found").and_then(|v| v.as_bool()) {
        let msg = data["error"].as_str().unwrap_or("Not found").to_string();
        return Err(AppError::NotFound(msg));
    }

    // Check if orchestra reported payload too large.
    if let Some(true) = data.get("too_large").and_then(|v| v.as_bool()) {
        let msg = data["error"]
            .as_str()
            .unwrap_or("File exceeds size limit")
            .to_string();
        return Ok(PayloadTooLarge(msg).into_response());
    }

    let result: BlobResponse = serde_json::from_value(data)
        .map_err(|e| AppError::Internal(format!("failed to parse source blob response: {e}")))?;

    Ok(Json(result).into_response())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_ok() {
        assert!(validate_path("src/main.rs").is_ok());
        assert!(validate_path("").is_ok());
        assert!(validate_path("a/b/c/d.txt").is_ok());
        assert_eq!(validate_path("src/main.rs").unwrap(), "src/main.rs");
        assert_eq!(validate_path("").unwrap(), "");
    }

    #[test]
    fn test_validate_path_traversal_rejected() {
        assert!(validate_path("../secret").is_err());
        assert!(validate_path("src/../../etc/passwd").is_err());
        assert!(validate_path("a/../../../b").is_err());
    }

    #[test]
    fn test_validate_path_absolute_rejected() {
        assert!(validate_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_path_dot_normalised() {
        // Current-dir components should be stripped cleanly (no error).
        assert!(validate_path("./src/main.rs").is_ok());
    }
}
