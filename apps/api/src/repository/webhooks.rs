use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id};

pub async fn create_webhook(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateWebhook,
) -> Result<Webhook, AppError> {
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let webhook = sqlx::query_as::<_, Webhook>(
        "INSERT INTO diraigent.webhook (project_id, name, url, secret, events, metadata)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(project_id)
    .bind(&req.name)
    .bind(&req.url)
    .bind(&req.secret)
    .bind(&req.events)
    .bind(&metadata)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => AppError::Conflict(format!(
            "Webhook '{}' already exists in this project",
            req.name
        )),
        _ => e.into(),
    })?;

    Ok(webhook)
}

pub async fn get_webhook(pool: &PgPool, id: Uuid) -> Result<Webhook, AppError> {
    fetch_by_id(pool, Table::Webhook, id, "Webhook not found").await
}

pub async fn list_webhooks(pool: &PgPool, project_id: Uuid) -> Result<Vec<Webhook>, AppError> {
    let webhooks = sqlx::query_as::<_, Webhook>(
        "SELECT * FROM diraigent.webhook WHERE project_id = $1 ORDER BY name",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(webhooks)
}

pub async fn update_webhook(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateWebhook,
) -> Result<Webhook, AppError> {
    let existing = get_webhook(pool, id).await?;
    let name = req.name.as_deref().unwrap_or(&existing.name);
    let url = req.url.as_deref().unwrap_or(&existing.url);
    let secret = req.secret.as_deref().or(existing.secret.as_deref());
    let events = req.events.as_ref().unwrap_or(&existing.events);
    let enabled = req.enabled.unwrap_or(existing.enabled);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let webhook = sqlx::query_as::<_, Webhook>(
        "UPDATE diraigent.webhook
         SET name = $2, url = $3, secret = $4, events = $5, enabled = $6, metadata = $7
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(url)
    .bind(secret)
    .bind(events)
    .bind(enabled)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(webhook)
}

pub async fn delete_webhook(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Webhook, id, "Webhook not found").await
}

pub async fn list_webhook_deliveries(
    pool: &PgPool,
    webhook_id: Uuid,
    limit: i64,
) -> Result<Vec<WebhookDelivery>, AppError> {
    let deliveries = sqlx::query_as::<_, WebhookDelivery>(
        "SELECT * FROM diraigent.webhook_delivery WHERE webhook_id = $1 ORDER BY delivered_at DESC LIMIT $2",
    )
    .bind(webhook_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(deliveries)
}

pub async fn list_webhook_dead_letters(
    pool: &PgPool,
    webhook_id: Uuid,
    limit: i64,
) -> Result<Vec<WebhookDeadLetter>, AppError> {
    let dead_letters = sqlx::query_as::<_, WebhookDeadLetter>(
        "SELECT * FROM diraigent.webhook_dead_letter WHERE webhook_id = $1 ORDER BY failed_at DESC LIMIT $2",
    )
    .bind(webhook_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(dead_letters)
}
