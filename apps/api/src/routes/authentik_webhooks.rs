//! Authentik webhook receiver for user lifecycle events.
//!
//! Receives webhook notifications from Authentik when users are deleted.
//! Validates the shared secret via the `Authorization` header and triggers
//! the same account cleanup as `DELETE /v1/account`.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::Deserialize;
use std::env;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/webhooks/authentik",
        axum::routing::post(receive_authentik_webhook),
    )
}

/// Authentik webhook notification payload.
///
/// Authentik's "Webhook" notification transport sends a JSON body with a
/// `model` object containing the affected user's details. The `pk` field
/// is the user's UUID (which Authentik uses as the `sub` claim in JWTs).
#[derive(Debug, Deserialize)]
struct AuthentikWebhookPayload {
    /// The event name, e.g. "model_deleted"
    #[serde(default)]
    event: Option<String>,
    /// The model object (the user being deleted)
    model: Option<AuthentikModel>,
}

#[derive(Debug, Deserialize)]
struct AuthentikModel {
    /// The user's UUID in Authentik (= JWT `sub` claim = auth_user.auth_user_id)
    pk: Option<String>,
}

/// POST /webhooks/authentik
///
/// Unauthenticated endpoint — security is provided by a shared secret
/// in the `Authorization: Bearer <secret>` header, configured via
/// `AUTHENTIK_WEBHOOK_SECRET` env var.
async fn receive_authentik_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AuthentikWebhookPayload>,
) -> impl IntoResponse {
    // Validate shared secret
    let expected_secret = match env::var("AUTHENTIK_WEBHOOK_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(s) => s,
        None => {
            tracing::warn!("Authentik webhook received but AUTHENTIK_WEBHOOK_SECRET is not set");
            return StatusCode::SERVICE_UNAVAILABLE;
        }
    };

    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    match provided {
        Some(token) if token == expected_secret => {}
        _ => {
            tracing::warn!("Authentik webhook: invalid or missing authorization");
            return StatusCode::UNAUTHORIZED;
        }
    }

    // Extract the user's Authentik UUID from the payload
    let auth_user_id = match payload
        .model
        .as_ref()
        .and_then(|m| m.pk.as_deref())
        .filter(|pk| !pk.is_empty())
    {
        Some(id) => id.to_string(),
        None => {
            tracing::warn!("Authentik webhook: missing model.pk in payload");
            return StatusCode::BAD_REQUEST;
        }
    };

    tracing::info!(
        auth_user_id = %auth_user_id,
        event = ?payload.event,
        "Authentik webhook: processing user deletion"
    );

    // Resolve the external auth_user_id to our internal user_id
    let user_id = match state.db.get_user_id_by_auth_id(&auth_user_id).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::info!(auth_user_id = %auth_user_id, "Authentik webhook: user not found locally, ignoring");
            return StatusCode::OK;
        }
        Err(e) => {
            tracing::error!(error = %e, "Authentik webhook: failed to look up user");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // Delete the account
    match state.db.delete_user_account(user_id).await {
        Ok(()) => {
            tracing::info!(user_id = %user_id, "Authentik webhook: user account deleted");
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!(error = %e, "Authentik webhook: failed to delete user account");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
