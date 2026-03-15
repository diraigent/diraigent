use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::{PlanSseEvent, PlannedTask};
use crate::ws_protocol::WsMessage;
use crate::ws_registry::GitResponsePayload;

pub fn routes() -> Router<AppState> {
    Router::new().route("/agents/{agent_id}/ws", get(ws_handler))
}

async fn ws_handler(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(agent_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, AppError> {
    let is_owner = state
        .db
        .verify_agent_owner(agent_id, user_id)
        .await
        .map_err(|_| AppError::Unauthorized("Agent ownership check failed".into()))?;
    if !is_owner {
        return Err(AppError::Forbidden("You do not own this agent".into()));
    }
    Ok(ws.on_upgrade(move |socket| handle_socket(state, agent_id, socket)))
}

async fn handle_socket(state: AppState, agent_id: Uuid, socket: WebSocket) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

    // Register this agent's connection
    state.ws_registry.register(agent_id, tx);

    // Writer task: reads from channel, sends to WS
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let text = match serde_json::to_string(&msg) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to serialize WS message");
                    continue;
                }
            };
            if ws_sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Reader task: reads from WS, routes messages
    while let Some(Ok(msg)) = ws_stream.next().await {
        match msg {
            Message::Text(text) => {
                let ws_msg: WsMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(error = %e, "malformed WS message from agent");
                        continue;
                    }
                };

                match ws_msg {
                    WsMessage::ChatEvent { session_id, event } => {
                        state.ws_registry.route_chat_event(&session_id, event).await;
                    }
                    WsMessage::GitResponse {
                        request_id,
                        success,
                        error,
                        data,
                    } => {
                        state.ws_registry.complete_git_request(
                            &request_id,
                            GitResponsePayload {
                                success,
                                error,
                                data,
                            },
                        );
                    }
                    WsMessage::PlanResponse {
                        request_id,
                        success,
                        error,
                        tasks,
                    } => {
                        let event = if success {
                            match serde_json::from_value::<Vec<PlannedTask>>(tasks) {
                                Ok(parsed) => PlanSseEvent::Done {
                                    tasks: parsed,
                                    success_criteria: None,
                                },
                                Err(e) => PlanSseEvent::Error {
                                    message: format!("Failed to parse plan: {e}"),
                                },
                            }
                        } else {
                            PlanSseEvent::Error {
                                message: error.unwrap_or_else(|| "Planning failed".into()),
                            }
                        };
                        state.ws_registry.route_plan_event(&request_id, event).await;
                    }
                    WsMessage::Heartbeat => {
                        let db = state.db.clone();
                        let aid = agent_id;
                        tokio::spawn(async move {
                            let _ = db.agent_heartbeat(aid, None).await;
                        });
                    }
                    _ => {
                        tracing::warn!("unexpected WS message type from agent");
                    }
                }
            }
            Message::Close(_) => break,
            _ => {} // ignore ping/pong/binary
        }
    }

    // Clean up
    state.ws_registry.unregister(agent_id);
    write_task.abort();
}
