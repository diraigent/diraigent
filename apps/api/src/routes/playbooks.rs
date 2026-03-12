use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::*;
use crate::repository;
use crate::tenant::TenantContext;

/// Playbook response that includes optional resolved_steps when step templates are used.
#[derive(Debug, Serialize)]
struct PlaybookResponse {
    #[serde(flatten)]
    playbook: Playbook,
    /// Steps with step_template_id references resolved (template defaults merged with inline overrides).
    /// Only present when at least one step has a step_template_id.
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_steps: Option<serde_json::Value>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/playbooks", post(create).get(list))
        .route("/playbooks/{id}", get(get_one).put(update).delete(remove))
        .route("/playbooks/{id}/sync", post(sync))
        .route("/git-strategies", get(git_strategies))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Json(req): Json<CreatePlaybook>,
) -> Result<Json<Playbook>, AppError> {
    // Validate step_template_id references before creating
    if let Some(ref steps) = req.steps {
        repository::validate_step_template_ids(&state.pool, steps).await?;
    }
    let p = state
        .db
        .create_playbook(tenant.tenant_id, &req, user_id)
        .await?;
    Ok(Json(p))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    tenant: TenantContext,
    Query(filters): Query<PlaybookFilters>,
) -> Result<Json<Vec<Playbook>>, AppError> {
    let items = state.db.list_playbooks(tenant.tenant_id, &filters).await?;
    Ok(Json(items))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<Json<PlaybookResponse>, AppError> {
    let p = state.db.get_playbook_by_id(id).await?;
    // Default (shared) playbooks are readable by all; tenant-owned must match.
    if let Some(tid) = p.tenant_id
        && tid != tenant.tenant_id
    {
        return Err(AppError::Forbidden(
            "Playbook belongs to another tenant".into(),
        ));
    }

    // Check if any step references a template — only resolve if needed.
    let has_template_refs = p
        .steps
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .any(|s| s.get("step_template_id").is_some_and(|v| !v.is_null()))
        })
        .unwrap_or(false);

    let resolved_steps = if has_template_refs {
        Some(repository::resolve_playbook_steps(&state.pool, &p.steps).await)
    } else {
        None
    };

    Ok(Json(PlaybookResponse {
        playbook: p,
        resolved_steps,
    }))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePlaybook>,
) -> Result<Json<Playbook>, AppError> {
    // Validate step_template_id references before updating
    if let Some(ref steps) = req.steps {
        repository::validate_step_template_ids(&state.pool, steps).await?;
    }
    let existing = state.db.get_playbook_by_id(id).await?;
    if existing.tenant_id.is_none() {
        // Default (shared) playbook is immutable — fork it for this tenant with the
        // requested changes applied on top of the source.
        let p = state
            .db
            .fork_playbook(tenant.tenant_id, &existing, &req, user_id)
            .await?;
        return Ok(Json(p));
    }
    let p = state.db.update_playbook(id, &req).await?;
    Ok(Json(p))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let existing = state.db.get_playbook_by_id(id).await?;
    if existing.tenant_id.is_none() {
        return Err(AppError::Forbidden(
            "default playbooks are immutable and cannot be deleted".to_string(),
        ));
    }
    state.db.delete_playbook(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Re-sync a forked playbook with its parent's latest content.
/// Only works on tenant-owned playbooks that have a parent_id.
async fn sync(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Playbook>, AppError> {
    let existing = state.db.get_playbook_by_id(id).await?;
    if existing.tenant_id.is_none() {
        return Err(AppError::Validation(
            "cannot sync a default playbook — it is already the source".to_string(),
        ));
    }
    let p = state.db.sync_playbook_with_parent(id).await?;
    Ok(Json(p))
}

/// Static catalog of immutable git strategies.
///
/// These are fixed options that can be selected per-playbook via `metadata.git_strategy`.
async fn git_strategies() -> Json<serde_json::Value> {
    Json(serde_json::json!([
        {
            "id": "merge_to_default",
            "name": "Merge to Default Branch",
            "description": "Branch from default, merge back when done. Standard autonomous workflow.",
        },
        {
            "id": "branch_only",
            "name": "Branch Only (No Merge)",
            "description": "Branch from default, push branch to origin. No automatic merge. For PR-based workflows.",
        },
        {
            "id": "branch_to_target",
            "name": "Merge to Target Branch",
            "description": "Branch from and merge to a specified target branch (e.g. develop, staging).",
            "fields": { "git_target_branch": "string" },
        },
        {
            "id": "no_git",
            "name": "No Git",
            "description": "Plain directory, no git operations. For non-code tasks.",
        },
    ]))
}
