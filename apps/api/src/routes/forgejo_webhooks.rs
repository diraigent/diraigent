//! Forgejo webhook receiver endpoint.
//!
//! Receives webhook events from Forgejo, validates the HMAC-SHA256 signature
//! against the stored webhook secret, and routes supported event types
//! (`workflow_run`, `workflow_job`) to the CI ingestion layer.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/webhooks/forgejo/{integration_id}",
            axum::routing::post(receive_forgejo_webhook),
        )
        .route(
            "/{project_id}/forgejo/sync",
            axum::routing::post(sync_forgejo_runs),
        )
}

/// Verify the Forgejo HMAC-SHA256 signature using constant-time comparison.
///
/// Forgejo sends the signature in the `X-Gitea-Signature` header as a raw hex
/// string (no `sha256=` prefix).
fn verify_signature(secret: &str, payload: &[u8], signature_hex: &str) -> bool {
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(payload);

    // Decode the hex signature from the header
    let Ok(expected_bytes) = hex::decode(signature_hex) else {
        return false;
    };

    // constant-time comparison via the `hmac` crate's `verify_slice`
    mac.verify_slice(&expected_bytes).is_ok()
}

/// POST /webhooks/forgejo/{integration_id}
///
/// This endpoint is **unauthenticated** (no JWT required). Security is provided
/// by the HMAC-SHA256 signature in the `X-Gitea-Signature` header, validated
/// against the stored webhook secret for the integration.
async fn receive_forgejo_webhook(
    State(state): State<AppState>,
    Path(integration_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // 1. Look up the integration
    let integration = match state.db.get_forgejo_integration(integration_id).await {
        Ok(i) => i,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Integration not found"})),
            );
        }
    };

    if !integration.enabled {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Integration not found"})),
        );
    }

    // 2. Validate HMAC signature
    let signature = headers
        .get("X-Gitea-Signature")
        .and_then(|v| v.to_str().ok());

    let Some(webhook_secret) = &integration.webhook_secret else {
        // No secret configured — reject (we require HMAC validation)
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Webhook secret not configured"})),
        );
    };

    let Some(signature) = signature else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Missing X-Gitea-Signature header"})),
        );
    };

    if !verify_signature(webhook_secret, &body, signature) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid signature"})),
        );
    }

    // 3. Parse the event type
    let event_type = headers
        .get("X-Gitea-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // 4. Route based on event type
    match event_type {
        "workflow_run" => {
            let payload: WorkflowRunEvent = match serde_json::from_slice(&body) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse workflow_run payload");
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "Invalid JSON payload"})),
                    );
                }
            };
            handle_workflow_run(&state, integration.project_id, payload).await;
        }
        "workflow_job" => {
            let payload: WorkflowJobEvent = match serde_json::from_slice(&body) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse workflow_job payload");
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "Invalid JSON payload"})),
                    );
                }
            };
            handle_workflow_job(&state, &integration, payload).await;
        }
        _ => {
            // Unrecognised event type — acknowledge gracefully
            tracing::debug!(event_type, "Unrecognised Forgejo event type, ignoring");
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

// ── Forgejo webhook event payloads ──

#[derive(Debug, Deserialize)]
struct WorkflowRunEvent {
    workflow_run: WorkflowRunPayload,
}

#[derive(Debug, Deserialize)]
struct WorkflowRunPayload {
    id: i64,
    name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    head_branch: Option<String>,
    head_sha: Option<String>,
    #[serde(default)]
    triggering_actor: Option<TriggeringActor>,
    run_started_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct TriggeringActor {
    login: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowJobEvent {
    workflow_job: WorkflowJobPayload,
}

#[derive(Debug, Deserialize)]
struct WorkflowJobPayload {
    id: i64,
    run_id: i64,
    #[serde(default)]
    name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    #[serde(default)]
    runner_name: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

/// Process a workflow_run event and upsert a ci_run record.
async fn handle_workflow_run(state: &AppState, project_id: Uuid, event: WorkflowRunEvent) {
    let run = &event.workflow_run;

    // Derive a single status string: prefer conclusion (success/failure/cancelled)
    // when present; otherwise fall back to status (queued/in_progress/completed).
    let status = run
        .conclusion
        .as_deref()
        .filter(|c| !c.is_empty())
        .or(run.status.as_deref())
        .unwrap_or("unknown");

    let workflow_name = run.name.as_deref().unwrap_or("unknown");
    let triggered_by = run
        .triggering_actor
        .as_ref()
        .and_then(|a| a.login.as_deref());

    // Derive finished_at: if status indicates completion, use updated_at
    let finished_at = match status {
        "success" | "failure" | "cancelled" => run.updated_at,
        _ => None,
    };

    match state
        .db
        .upsert_ci_run(
            project_id,
            run.id,
            workflow_name,
            status,
            run.head_branch.as_deref(),
            run.head_sha.as_deref(),
            triggered_by,
            run.run_started_at,
            finished_at,
            "forgejo",
        )
        .await
    {
        Ok(ci_run) => {
            tracing::info!(
                ci_run_id = %ci_run.id,
                external_id = run.id,
                status,
                "Upserted ci_run from webhook"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, external_id = run.id, "Failed to upsert ci_run");
        }
    }
}

/// Process a workflow_job event: upsert the job record and fetch+store steps.
async fn handle_workflow_job(
    state: &AppState,
    integration: &crate::models::ForgejoIntegration,
    event: WorkflowJobEvent,
) {
    let job = &event.workflow_job;
    let project_id = integration.project_id;

    // 1. Find or create the parent ci_run
    let ci_run = match state
        .db
        .get_ci_run_by_external_id(project_id, "forgejo", job.run_id)
        .await
    {
        Ok(Some(run)) => run,
        Ok(None) => {
            // Create a stub ci_run — the workflow_run event may arrive later
            match state
                .db
                .upsert_ci_run(
                    project_id,
                    job.run_id,
                    "unknown",
                    "in_progress",
                    None,
                    None,
                    None,
                    None,
                    None,
                    "forgejo",
                )
                .await
            {
                Ok(run) => run,
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        external_id = job.run_id,
                        "Failed to create stub ci_run for workflow_job"
                    );
                    return;
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to look up ci_run for workflow_job");
            return;
        }
    };

    // 2. Derive status
    let status = job
        .conclusion
        .as_deref()
        .filter(|c| !c.is_empty())
        .or(job.status.as_deref())
        .unwrap_or("unknown");

    let job_name = job.name.as_deref().unwrap_or("unknown");

    // 3. Upsert the job record
    let ci_job = match state
        .db
        .upsert_ci_job_by_name(
            ci_run.id,
            job_name,
            status,
            job.runner_name.as_deref(),
            job.started_at,
            job.completed_at,
        )
        .await
    {
        Ok(j) => j,
        Err(e) => {
            tracing::error!(error = %e, job_name, "Failed to upsert ci_job");
            return;
        }
    };

    tracing::info!(
        ci_job_id = %ci_job.id,
        ci_run_id = %ci_run.id,
        job_name,
        status,
        "Upserted ci_job from webhook"
    );

    // 4. Fetch step details from the Forgejo API and store them
    fetch_and_store_steps(
        state,
        integration,
        project_id,
        job.run_id,
        job.id,
        ci_job.id,
    )
    .await;
}

/// Extract owner/repo from a repository URL.
///
/// Handles URLs like:
/// - `https://git.example.com/owner/repo`
/// - `https://git.example.com/owner/repo.git`
/// - `git@git.example.com:owner/repo.git`
fn extract_owner_repo(repo_url: &str) -> Option<(String, String)> {
    let path = repo_url.trim_end_matches('/').trim_end_matches(".git");

    // Handle SSH-style URLs (git@host:owner/repo)
    let path = if let Some(colon_part) = path.strip_prefix("git@") {
        // git@host:owner/repo → owner/repo
        colon_part.split_once(':')?.1
    } else {
        path
    };

    let parts: Vec<&str> = path.rsplitn(3, '/').collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[1].to_string(), parts[0].to_string()))
    } else {
        None
    }
}

/// Fetch step details from the Forgejo API for a given job and store them.
///
/// Failed API calls are logged and do NOT abort the job upsert.
async fn fetch_and_store_steps(
    state: &AppState,
    integration: &crate::models::ForgejoIntegration,
    project_id: Uuid,
    external_run_id: i64,
    external_job_id: i64,
    ci_job_id: Uuid,
) {
    // We need the project's repo_url to derive owner/repo
    let project = match state.db.get_project_by_id(project_id).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to look up project for step fetch; skipping steps");
            return;
        }
    };

    let repo_url = match &project.repo_url {
        Some(url) if !url.is_empty() => url.as_str(),
        _ => {
            tracing::warn!(
                project_id = %project_id,
                "Project has no repo_url; cannot fetch steps from Forgejo API"
            );
            return;
        }
    };

    let (owner, repo) = match extract_owner_repo(repo_url) {
        Some(pair) => pair,
        None => {
            tracing::warn!(
                repo_url,
                "Could not extract owner/repo from repo_url; skipping steps"
            );
            return;
        }
    };

    // Build a Forgejo API client from the integration config
    let client =
        forgejo_client::ForgejoClient::new(&integration.base_url, integration.token.clone());

    // Fetch steps from the API
    let steps = match client
        .list_steps(&owner, &repo, external_run_id, external_job_id)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                error = %e,
                external_run_id,
                external_job_id,
                "Failed to fetch steps from Forgejo API; job upsert was not aborted"
            );
            return;
        }
    };

    // Delete existing steps for this job (idempotent replacement)
    if let Err(e) = state.db.delete_steps_for_job(ci_job_id).await {
        tracing::error!(error = %e, ci_job_id = %ci_job_id, "Failed to delete old steps");
        return;
    }

    // Insert new steps
    for step in &steps {
        let step_status = step
            .conclusion
            .as_deref()
            .filter(|c| !c.is_empty())
            .unwrap_or(&step.status);

        if let Err(e) = state
            .db
            .insert_ci_step(
                ci_job_id,
                &step.name,
                step_status,
                None, // exit_code is not provided by the Forgejo API
                step.started_at,
                step.completed_at,
            )
            .await
        {
            tracing::warn!(
                error = %e,
                step_name = step.name,
                "Failed to insert ci_step; continuing with remaining steps"
            );
        }
    }

    tracing::info!(
        ci_job_id = %ci_job_id,
        step_count = steps.len(),
        "Stored steps from Forgejo API"
    );
}

// ── Manual sync endpoint ──

/// POST /{project_id}/forgejo/sync
///
/// Manually sync recent CI runs from the Forgejo API for backfill.
/// Requires authentication and project membership.
async fn sync_forgejo_runs(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> impl IntoResponse {
    // Verify membership
    if let Err(e) = require_membership(state.db.as_ref(), agent_id, user_id, project_id).await {
        return e.into_response();
    }

    // Look up the integration
    let integration = match state
        .db
        .get_forgejo_integration_by_project(project_id)
        .await
    {
        Ok(i) if i.enabled => i,
        Ok(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Forgejo integration is disabled"})),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "No Forgejo integration found for this project"})),
            )
                .into_response();
        }
    };

    // Get project for repo_url
    let project = match state.db.get_project_by_id(project_id).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to load project: {e}")})),
            )
                .into_response();
        }
    };

    let repo_url = match &project.repo_url {
        Some(url) if !url.is_empty() => url.as_str(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Project has no repo_url configured"})),
            )
                .into_response();
        }
    };

    let (owner, repo) = match extract_owner_repo(repo_url) {
        Some(pair) => pair,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Could not extract owner/repo from repo_url"})),
            )
                .into_response();
        }
    };

    let client =
        forgejo_client::ForgejoClient::new(&integration.base_url, integration.token.clone());

    // Fetch the last N runs (2 pages of 25 = up to 50 runs)
    let mut synced_count: usize = 0;
    let mut errors: usize = 0;

    for page in 1..=2u32 {
        let runs = match client.list_runs(&owner, &repo, page, 25).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, page, "Failed to fetch runs from Forgejo API");
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": format!("Failed to fetch runs from Forgejo API: {e}"),
                        "synced": synced_count
                    })),
                )
                    .into_response();
            }
        };

        if runs.workflow_runs.is_empty() {
            break;
        }

        for run in &runs.workflow_runs {
            let status = run
                .conclusion
                .as_deref()
                .filter(|c| !c.is_empty())
                .unwrap_or(&run.status);

            let triggered_by = run.triggering_actor.as_ref().map(|a| a.login.as_str());

            let finished_at = match status {
                "success" | "failure" | "cancelled" => run.updated_at,
                _ => None,
            };

            match state
                .db
                .upsert_ci_run(
                    project_id,
                    run.id,
                    &run.name,
                    status,
                    Some(run.head_branch.as_str()),
                    Some(run.head_sha.as_str()),
                    triggered_by,
                    run.run_started_at,
                    finished_at,
                    "forgejo",
                )
                .await
            {
                Ok(_) => synced_count += 1,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        external_id = run.id,
                        "Failed to upsert run during sync; continuing"
                    );
                    errors += 1;
                }
            }
        }

        // Stop if we got fewer results than the page size
        if runs.workflow_runs.len() < 25 {
            break;
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "synced": synced_count,
            "errors": errors
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature_valid() {
        let secret = "test-secret-key";
        let payload = b"hello world";

        // Compute expected signature
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());

        assert!(verify_signature(secret, payload, &signature));
    }

    #[test]
    fn test_verify_signature_invalid() {
        let secret = "test-secret-key";
        let payload = b"hello world";

        assert!(!verify_signature(secret, payload, "0000deadbeef"));
    }

    #[test]
    fn test_verify_signature_bad_hex() {
        let secret = "test-secret-key";
        let payload = b"hello world";

        assert!(!verify_signature(secret, payload, "not-valid-hex!!!"));
    }

    #[test]
    fn test_extract_owner_repo_https() {
        let (owner, repo) = extract_owner_repo("https://git.example.com/alice/myrepo").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_extract_owner_repo_trailing_slash() {
        let (owner, repo) = extract_owner_repo("https://git.example.com/alice/myrepo/").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_extract_owner_repo_dot_git() {
        let (owner, repo) = extract_owner_repo("https://git.example.com/alice/myrepo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_extract_owner_repo_ssh() {
        let (owner, repo) = extract_owner_repo("git@git.example.com:alice/myrepo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "myrepo");
    }

    #[test]
    fn test_extract_owner_repo_none_for_bare() {
        assert!(extract_owner_repo("https://example.com").is_none());
    }

    #[test]
    fn test_verify_signature_wrong_secret() {
        let secret = "correct-secret";
        let payload = b"hello world";

        let mut mac = Hmac::<Sha256>::new_from_slice(b"wrong-secret").unwrap();
        mac.update(payload);
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());

        assert!(!verify_signature(secret, payload, &signature));
    }
}
