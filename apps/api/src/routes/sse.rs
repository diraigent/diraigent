use axum::Router;
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, http::StatusCode};
use futures::StreamExt;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/review/stream/ticket", post(issue_ticket))
        .route("/review/stream", get(review_stream))
        .route("/agents/stream/ticket", post(issue_agent_ticket))
        .route("/agents/stream", get(agent_stream))
}

/// Request a short-lived opaque ticket for the SSE stream.
///
/// The browser `EventSource` API cannot set custom headers, so a Bearer token
/// must not be placed directly in the URL. Instead, the client:
/// 1. Calls this endpoint with a normal `Authorization: Bearer` header to get a ticket.
/// 2. Opens the EventSource with `?ticket=<uuid>` — an opaque, single-use, 60-second token.
///
/// This keeps the full JWT out of server logs, browser history, and proxy logs.
async fn issue_ticket(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<TicketResponse>, StatusCode> {
    let ticket = state.sse_tickets.issue(user_id).await;
    Ok(Json(TicketResponse { ticket }))
}

#[derive(Serialize)]
struct TicketResponse {
    ticket: Uuid,
}

#[derive(Deserialize)]
struct TicketQuery {
    ticket: Uuid,
}

/// SSE endpoint that streams `review_update` events whenever a task enters or
/// leaves `human_review`.  The web client subscribes on page load instead of
/// polling every 30 s.
///
/// Authentication: short-lived opaque ticket obtained from
/// `POST /review/stream/ticket`. The ticket is consumed on first use and
/// expires after 60 seconds.
async fn review_stream(
    State(state): State<AppState>,
    Query(params): Query<TicketQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    // Consume the ticket — single-use, 60-second TTL.
    state
        .sse_tickets
        .consume(params.ticket)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let rx = state.review_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).ok()?;
                Some(Ok::<Event, Infallible>(
                    Event::default().event("review_update").data(data),
                ))
            }
            // Lagged: subscriber fell behind; skip rather than disconnect.
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

/// Issue a short-lived ticket for the agent status SSE stream.
async fn issue_agent_ticket(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<TicketResponse>, StatusCode> {
    let ticket = state.sse_tickets.issue(user_id).await;
    Ok(Json(TicketResponse { ticket }))
}

/// SSE endpoint that streams `agent_update` events whenever an agent's status
/// changes (heartbeat, update). The web client subscribes instead of polling
/// every 30 s to keep the agent-indicator accurate in real time.
///
/// Authentication: short-lived opaque ticket from `POST /agents/stream/ticket`.
async fn agent_stream(
    State(state): State<AppState>,
    Query(params): Query<TicketQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    state
        .sse_tickets
        .consume(params.ticket)
        .await
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let rx = state.agent_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).ok()?;
                Some(Ok::<Event, Infallible>(
                    Event::default().event("agent_update").data(data),
                ))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}
