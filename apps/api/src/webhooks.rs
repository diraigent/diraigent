use crate::db::DiraigentDb;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use std::sync::Arc;
use uuid::Uuid;

const MAX_ATTEMPTS: usize = 5;
const BACKOFF_MS: [u64; MAX_ATTEMPTS] = [1_000, 5_000, 30_000, 120_000, 600_000];

#[derive(Clone)]
pub struct WebhookDispatcher {
    client: Client,
    db: Arc<dyn DiraigentDb>,
}

impl WebhookDispatcher {
    pub fn new(db: Arc<dyn DiraigentDb>) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            db,
        }
    }

    pub fn fire(&self, project_id: Uuid, event_type: &str, payload: serde_json::Value) {
        let dispatcher = self.clone();
        let event_type = event_type.to_string();
        tokio::spawn(async move {
            if let Err(e) = dispatcher.dispatch(project_id, &event_type, payload).await {
                tracing::warn!(error = %e, "Webhook dispatch failed");
            }
        });
    }

    async fn dispatch(
        &self,
        project_id: Uuid,
        event_type: &str,
        payload: serde_json::Value,
    ) -> anyhow::Result<()> {
        let webhooks = self.db.list_webhooks_enabled(project_id).await?;

        for webhook in webhooks {
            if !webhook.events.contains(&"*".to_string())
                && !webhook.events.contains(&event_type.to_string())
            {
                continue;
            }

            let body = serde_json::json!({
                "event": event_type,
                "project_id": project_id,
                "payload": payload,
                "timestamp": chrono::Utc::now(),
            });

            let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
            let signature = webhook
                .secret
                .as_deref()
                .map(|s| compute_signature(s, &body_bytes));

            let success = deliver_with_retry(
                &self.client,
                self.db.as_ref(),
                webhook.id,
                &webhook.url,
                &body_bytes,
                signature.as_deref(),
                event_type,
                &body,
            )
            .await;

            crate::metrics::record_webhook_delivery(success);
        }

        Ok(())
    }
}

pub fn compute_signature(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(payload);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

#[allow(clippy::too_many_arguments)]
pub async fn deliver_with_retry(
    client: &Client,
    db: &dyn DiraigentDb,
    webhook_id: Uuid,
    url: &str,
    body: &[u8],
    signature: Option<&str>,
    event_type: &str,
    payload_json: &serde_json::Value,
) -> bool {
    let mut last_status: Option<i32> = None;
    let mut last_body: Option<String> = None;

    for (attempt, backoff_ms) in BACKOFF_MS.iter().enumerate().take(MAX_ATTEMPTS) {
        let attempt_number = (attempt + 1) as i32;

        let mut request = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-Webhook-Event", event_type)
            .body(body.to_vec());

        if let Some(sig) = signature {
            request = request.header("X-Webhook-Signature", sig);
        }

        let (status, resp_body, success) = match request.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16() as i32;
                let resp_body = resp.text().await.unwrap_or_default();
                let success = (200..300).contains(&(status as usize));
                (Some(status), Some(resp_body), success)
            }
            Err(e) => (None, Some(format!("Error: {}", e)), false),
        };

        db.record_webhook_delivery(
            webhook_id,
            event_type,
            payload_json,
            status,
            resp_body.as_deref(),
            success,
            attempt_number,
        )
        .await;

        if success {
            return true;
        }

        last_status = status;
        last_body = resp_body;

        if let Some(s) = status
            && s < 500
        {
            break;
        }

        if attempt < MAX_ATTEMPTS - 1 {
            tokio::time::sleep(std::time::Duration::from_millis(*backoff_ms)).await;
        }
    }

    db.record_webhook_dead_letter(
        webhook_id,
        event_type,
        payload_json,
        last_status,
        last_body.as_deref(),
        MAX_ATTEMPTS as i32,
    )
    .await;

    false
}
