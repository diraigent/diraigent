use axum::extract::{Path, State};
use axum::routing::{get, post};
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
        .route(
            "/{project_id}/webhooks",
            post(create_webhook).get(list_webhooks),
        )
        .route(
            "/webhooks/{id}",
            get(get_webhook).put(update_webhook).delete(delete_webhook),
        )
        .route("/webhooks/{id}/deliveries", get(list_deliveries))
        .route("/webhooks/{id}/dead-letters", get(list_dead_letters))
        .route("/webhooks/{id}/test", post(test_webhook))
}

async fn create_webhook(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateWebhook>,
) -> Result<Json<Webhook>, AppError> {
    validation::validate_create_webhook(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let webhook = state.db.create_webhook(project_id, &req).await?;
    Ok(Json(webhook))
}

async fn list_webhooks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Webhook>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let webhooks = state.db.list_webhooks(project_id).await?;
    Ok(Json(webhooks))
}

async fn get_webhook(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Webhook>, AppError> {
    let webhook = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_webhook(id).await?,
    )
    .await?;
    Ok(Json(webhook))
}

async fn update_webhook(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWebhook>,
) -> Result<Json<Webhook>, AppError> {
    validation::validate_update_webhook(&req)?;
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_webhook(id).await?,
        "manage",
    )
    .await?;
    let webhook = state.db.update_webhook(id, &req).await?;
    Ok(Json(webhook))
}

async fn delete_webhook(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_webhook(id).await?,
        "manage",
    )
    .await?;
    state.db.delete_webhook(id).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

async fn list_deliveries(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<WebhookDelivery>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_webhook(id).await?,
    )
    .await?;
    let deliveries = state.db.list_webhook_deliveries(id, 50).await?;
    Ok(Json(deliveries))
}

async fn list_dead_letters(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<WebhookDeadLetter>>, AppError> {
    ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_webhook(id).await?,
    )
    .await?;
    let dead_letters = state.db.list_webhook_dead_letters(id, 50).await?;
    Ok(Json(dead_letters))
}

async fn test_webhook(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let webhook = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_webhook(id).await?,
        "manage",
    )
    .await?;
    state.webhooks.fire(
        webhook.project_id,
        "webhook.test",
        serde_json::json!({"webhook_id": id, "message": "Test delivery"}),
    );
    Ok(Json(serde_json::json!({"status": "sent"})))
}
