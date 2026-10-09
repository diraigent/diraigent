use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use futures::stream::Stream;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::chat::{self, ChatSseEvent, ChatStreamParams, Message};
use crate::error::AppError;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/chat", post(chat_handler))
        .route("/{project_id}/chat/models", get(chat_models))
}

#[derive(Default, Deserialize)]
struct ModelQuery {
    #[serde(default)]
    refresh: bool,
}

struct PendingCatalog {
    registry: std::sync::Arc<crate::ws_registry::WsRegistry>,
    request_id: String,
}

impl Drop for PendingCatalog {
    fn drop(&mut self) {
        self.registry.remove_model_request(&self.request_id);
    }
}

async fn chat_models(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ModelQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let project = state.db.get_project_by_id(project_id).await?;
    let candidates = match crate::project_content::owner(&state, project_id).await? {
        Some(id) => {
            crate::project_content::require_active_owner(&state, project_id, id).await?;
            vec![id]
        }
        None => state.db.list_tenant_agent_ids(project.tenant_id).await?,
    };
    // Same candidate list and selection as project chat.
    let worker = state
        .ws_registry
        .find_connected_agent(&candidates)
        .ok_or_else(|| {
            AppError::ServiceUnavailable(
                "No orchestra worker connected. Enter a model manually or try again later.".into(),
            )
        })?;
    let request_id = Uuid::now_v7().to_string();
    let rx = state
        .ws_registry
        .register_model_request(request_id.clone(), worker);
    let _pending = PendingCatalog {
        registry: state.ws_registry.clone(),
        request_id: request_id.clone(),
    };
    let sent = state.ws_registry.send_to_agent(
        worker,
        crate::ws_protocol::WsMessage::ChatModelsRequest {
            request_id: request_id.clone(),
            project_id,
            refresh: query.refresh,
        },
    );
    let result = if sent {
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(catalog))) => Ok(catalog),
            _ => Err(AppError::ServiceUnavailable(
                "Model discovery unavailable. Enter a model manually or refresh the list.".into(),
            )),
        }
    } else {
        Err(AppError::ServiceUnavailable(
            "Orchestra worker disconnected.".into(),
        ))
    };
    let mut data = serde_json::to_value(result?)
        .map_err(|_| AppError::Internal("Invalid model catalog".into()))?;
    data["agent_id"] = serde_json::json!(worker);
    Ok(Json(data))
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    messages: Vec<Message>,
    #[serde(default)]
    history_revision: Option<i64>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent_id: Option<Uuid>,
}

async fn chat_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let storage_owner = crate::project_content::owner(&state, project_id).await?;
    if let Some(owner) = storage_owner {
        crate::project_content::require_active_owner(&state, project_id, owner).await?;
    }
    let content_store_id = crate::project_content::store_id(&state, project_id).await?;
    if storage_owner.is_some() && req.agent_id.is_some_and(|id| Some(id) != storage_owner) {
        return Err(AppError::Conflict(
            "Refresh models from the project storage owner".into(),
        ));
    }
    if storage_owner.is_some() && req.history_revision.is_none() {
        return Err(AppError::Conflict(
            "Load the Orchestra conversation history before sending".into(),
        ));
    }

    // Derive the API base URL from the incoming request so the chat agent
    // knows the correct address even when the API is running remotely.
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8082");
    let api_base = format!("{scheme}://{host}");

    // Pass the caller's auth header through so the chat assistant can make
    // authenticated API calls (e.g. create tasks). Fall back to X-Dev-User-Id
    // in dev environments where no Authorization header is present.
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|h| format!("Authorization: {h}"))
        .unwrap_or_else(|| format!("X-Dev-User-Id: {user_id}"));

    let (tx, rx) = mpsc::channel::<ChatSseEvent>(64);
    let tx_watch = tx.clone();

    // Use a oneshot so the spawned chat task can report its session_id back.
    let (sid_tx, sid_rx) = tokio::sync::oneshot::channel::<String>();
    let ws_registry = state.ws_registry.clone();

    tokio::spawn(async move {
        let session_id = chat::run_chat_stream(ChatStreamParams {
            db: state.db.clone(),
            ws_registry: state.ws_registry.clone(),
            project_id,
            user_id,
            messages: req.messages,
            model: req.model,
            model_agent_id: storage_owner.or(req.agent_id),
            history_revision: req.history_revision,
            persist_history: storage_owner.is_some(),
            content_store_id,
            tx,
            api_base,
            auth_header,
        })
        .await;
        if let Some(sid) = session_id {
            let _ = sid_tx.send(sid);
        }
    });

    // Spawn a watcher that cancels the orchestra subprocess when the SSE
    // client disconnects. We await `tx_watch.closed()` which resolves
    // instantly when the receiver is dropped (client gone), giving us
    // zero-latency disconnect detection instead of polling.
    let ws_registry_cancel = ws_registry.clone();
    tokio::spawn(async move {
        // Wait for session_id, but also watch for early client disconnect.
        let session_id = tokio::select! {
            result = sid_rx => {
                match result {
                    Ok(sid) => sid,
                    Err(_) => return, // chat stream failed before registering
                }
            }
            _ = tx_watch.closed() => {
                // Client disconnected before session was registered;
                // the chat stream will see tx is closed and stop on its own.
                return;
            }
        };
        // Wait for the SSE client to disconnect (receiver dropped).
        tx_watch.closed().await;
        if ws_registry_cancel.is_chat_active(&session_id) {
            tracing::info!(session_id, "SSE client disconnected, cancelling chat");
            ws_registry_cancel.cancel_chat_session(&session_id);
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        let data = serde_json::to_string(&event).unwrap_or_default();
        let event_type = match &event {
            ChatSseEvent::Text { .. } => "text",
            ChatSseEvent::Thinking { .. } => "thinking",
            ChatSseEvent::ToolStart { .. } => "tool_start",
            ChatSseEvent::ToolEnd { .. } => "tool_end",
            ChatSseEvent::Done { .. } => "done",
            ChatSseEvent::Error { .. } => "error",
        };
        Ok(Event::default().event(event_type).data(data))
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}
