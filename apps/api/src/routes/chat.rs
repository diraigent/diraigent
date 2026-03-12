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

    tokio::spawn(chat::run_chat_stream(ChatStreamParams {
        db: state.db.clone(),
        ws_registry: state.ws_registry.clone(),
        project_id,
        user_id,
        messages: req.messages,
        tx,
        api_base,
        auth_header,
    }));

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
