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

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/webhooks/forgejo/{integration_id}",
        axum::routing::post(receive_forgejo_webhook),
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
            // Acknowledged but no-op for now; job-level ingestion can be added later
            if serde_json::from_slice::<serde_json::Value>(&body).is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "Invalid JSON payload"})),
                );
            }
            tracing::debug!(event_type, "workflow_job event received, acknowledged");
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
        )
        .await
    {
        Ok(ci_run) => {
            tracing::info!(
                ci_run_id = %ci_run.id,
                forgejo_run_id = run.id,
                status,
                "Upserted ci_run from webhook"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, forgejo_run_id = run.id, "Failed to upsert ci_run");
        }
    }
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
