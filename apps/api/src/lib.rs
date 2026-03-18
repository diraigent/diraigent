pub mod auth;
pub mod authz;
pub mod chat;
pub mod constants;
pub mod crypto;
pub mod csrf;
pub mod db;
pub mod error;
pub mod event_triggers;
pub mod metrics;
pub mod models;
pub mod openapi;
pub mod package_cache;
pub mod rate_limit;
pub mod repository;
pub mod routes;
pub mod scoring;
pub mod services;
pub mod stale_detector;
pub mod task_score;
pub mod tenant;
pub mod validation;
pub mod webhooks;
pub mod ws_protocol;
pub mod ws_registry;

use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Short-lived, single-use opaque tickets for SSE authentication.
///
/// The browser `EventSource` API cannot set custom headers, so the full JWT
/// must not appear in the URL. Instead, the web client calls
/// `POST /review/stream/ticket` (with a normal Bearer token) to obtain a
/// short-lived (60 s) opaque UUID. That UUID is passed as `?ticket=` when
/// opening the EventSource connection. Tickets are consumed on first use.
#[derive(Clone, Default)]
pub struct SseTicketStore {
    inner: Arc<RwLock<HashMap<Uuid, (Uuid, Instant)>>>,
}

impl SseTicketStore {
    pub const TICKET_TTL_SECS: u64 = 60;

    /// Issue a new ticket for `user_id`. Returns the ticket UUID.
    pub async fn issue(&self, user_id: Uuid) -> Uuid {
        let ticket = Uuid::now_v7();
        let mut store = self.inner.write().await;
        // Evict expired tickets to prevent unbounded growth.
        store.retain(|_, (_, created)| created.elapsed().as_secs() < Self::TICKET_TTL_SECS);
        store.insert(ticket, (user_id, Instant::now()));
        ticket
    }

    /// Consume a ticket. Returns `Some(user_id)` if the ticket is valid and
    /// not expired, `None` otherwise. Consumed tickets cannot be reused.
    pub async fn consume(&self, ticket: Uuid) -> Option<Uuid> {
        let mut store = self.inner.write().await;
        let (user_id, created) = store.remove(&ticket)?;
        if created.elapsed().as_secs() < Self::TICKET_TTL_SECS {
            Some(user_id)
        } else {
            None
        }
    }
}

/// Notification broadcast when a task enters or leaves `human_review`.
/// Used to drive the SSE review stream.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ReviewSseEvent {
    /// `"entered"` when a task transitions **to** human_review;
    /// `"left"` when it transitions **away** from human_review.
    pub kind: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub title: String,
}

/// Notification broadcast when an agent's status changes.
/// Used to drive the SSE agent status stream.
#[derive(Clone, Debug, serde::Serialize)]
pub struct AgentSseEvent {
    pub agent_id: Uuid,
    pub name: String,
    pub status: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn db::DiraigentDb>,
    /// Raw pool for operations that bypass the CryptoDb layer (e.g. key rotation).
    pub pool: PgPool,
    pub jwks: Arc<RwLock<auth::JwksCache>>,
    pub user_cache: auth::UserIdCache,
    pub webhooks: webhooks::WebhookDispatcher,
    pub repo_root: Option<PathBuf>,
    /// Parsed once at startup from `PRODUCTION` env var. When true, dev auth
    /// bypasses (DEV_USER_ID, X-Dev-User-Id) are disabled.
    pub is_production: bool,
    /// Base directory for all projects. When set, project `repo_path` values are
    /// interpreted relative to this directory, enabling path resolution and
    /// "open folder" style project creation.
    pub projects_path: Option<PathBuf>,
    pub loki_url: Option<String>,
    /// In-memory cache of project->package mappings to avoid N+1 DB queries
    /// when validating domain enum fields on bulk operations.
    pub pkg_cache: package_cache::PackageCache,
    /// Cache of decrypted DEKs per tenant (5-min TTL).
    pub dek_cache: crypto::DekCache,
    /// Embedding provider for semantic knowledge retrieval.
    /// Configured via EMBEDDING_URL / EMBEDDING_MODEL env vars.
    pub embedder: Arc<dyn services::embeddings::EmbeddingProvider>,
    /// Broadcast channel for human_review state changes -> SSE `/review/stream`.
    /// Subscribers are created per SSE connection; send errors are ignored when
    /// there are no active subscribers.
    pub review_tx: broadcast::Sender<ReviewSseEvent>,
    /// Broadcast channel for agent status changes -> SSE `/agents/stream`.
    /// Fired on heartbeat and status updates; frontend subscribes for real-time availability.
    pub agent_tx: broadcast::Sender<AgentSseEvent>,
    /// Short-lived opaque tickets issued by `POST /review/stream/ticket`.
    /// Consumed on first use; replaces the insecure `?token=` query param pattern.
    pub sse_tickets: SseTicketStore,
    /// WebSocket connection registry for orchestra agents.
    pub ws_registry: Arc<ws_registry::WsRegistry>,
}

impl AppState {
    /// Fire a project event: write an audit log entry and dispatch webhooks.
    ///
    /// Writes an audit log entry and dispatches webhooks asynchronously.
    #[allow(clippy::too_many_arguments)]
    pub fn fire_event(
        &self,
        project_id: Uuid,
        event_type: &str,
        entity_type: &str,
        entity_id: Uuid,
        actor_agent_id: Option<Uuid>,
        actor_user_id: Option<Uuid>,
        payload: serde_json::Value,
    ) {
        let db = self.db.clone();
        let webhooks = self.webhooks.clone();
        let event_type = event_type.to_string();
        let entity_type = entity_type.to_string();
        tokio::spawn(async move {
            // Audit log
            let summary = format!("{event_type} on {entity_type} {entity_id}");
            let _ = db
                .create_audit_entry(
                    project_id,
                    actor_agent_id,
                    actor_user_id,
                    &event_type,
                    &entity_type,
                    entity_id,
                    &summary,
                    None,
                    Some(&payload),
                )
                .await;
            // Webhook dispatch
            webhooks.fire(project_id, &event_type, payload);
        });
    }
}
