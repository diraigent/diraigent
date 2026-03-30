use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
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
use crate::chat::{self, ChatSseEvent, ChatStreamParams, Message};
use crate::error::AppError;

pub fn routes() -> Router<AppState> {
    Router::new().route("/{project_id}/chat", post(chat_handler))
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    messages: Vec<Message>,
    #[serde(default)]
    model: Option<String>,
}

async fn chat_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, AppError> {
    let _ = state.db.get_project_by_id(project_id).await?;

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
    // client disconnects. We monitor the mpsc sender: once the receiver is
    // dropped (client gone), `tx_watch.closed()` resolves and we cancel.
    let ws_registry_cancel = ws_registry.clone();
    tokio::spawn(async move {
        let session_id = match sid_rx.await {
            Ok(sid) => sid,
            Err(_) => return, // chat stream failed before registering a session
        };
        // Wait for the session to complete or be cancelled externally.
        // We can detect client disconnect by checking if the session is still
        // active in the registry — if the SSE receiver was dropped, events
        // sent via route_chat_event will fail. Poll periodically.
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            if !ws_registry_cancel.is_chat_active(&session_id) {
                // Session already completed or cancelled
                return;
            }
            // Check if the SSE client is gone by testing the sender
            if ws_registry_cancel.is_chat_sender_closed(&session_id) {
                tracing::info!(session_id, "SSE client disconnected, cancelling chat");
                ws_registry_cancel.cancel_chat_session(&session_id);
                return;
            }
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        let data = serde_json::to_string(&event).unwrap_or_default();
        let event_type = match &event {
            ChatSseEvent::Text { .. } => "text",
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
