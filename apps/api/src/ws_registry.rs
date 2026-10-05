use crate::chat::ChatSseEvent;
use crate::ws_protocol::WsMessage;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// Payload returned for completed git requests.
pub struct GitResponsePayload {
    pub success: bool,
    pub error: Option<String>,
    pub data: serde_json::Value,
}

type ModelReply = Result<diraigent_types::ChatModelCatalog, String>;
type PendingModelRequest = (Uuid, oneshot::Sender<ModelReply>);

struct ActiveChat {
    tx: mpsc::Sender<ChatSseEvent>,
    last_activity: tokio::time::Instant,
}

pub struct WsRegistry {
    /// Connected orchestras: agent_id -> WS sender
    connections: DashMap<Uuid, mpsc::UnboundedSender<WsMessage>>,
    /// Pending git requests: request_id -> oneshot sender
    pending_git: DashMap<String, oneshot::Sender<GitResponsePayload>>,
    pending_models: DashMap<String, PendingModelRequest>,
    /// Active chat sessions: session_id -> mpsc sender for SSE events
    active_chats: DashMap<String, ActiveChat>,
    /// Which agent handles each chat session: session_id -> agent_id
    session_agents: DashMap<String, Uuid>,
}

impl Default for WsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WsRegistry {
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
            pending_git: DashMap::new(),
            pending_models: DashMap::new(),
            active_chats: DashMap::new(),
            session_agents: DashMap::new(),
        }
    }

    /// Register a connected orchestra agent.
    pub fn register(&self, agent_id: Uuid, tx: mpsc::UnboundedSender<WsMessage>) {
        self.connections.insert(agent_id, tx);
        tracing::info!(%agent_id, "agent connected via WebSocket");
    }

    /// Unregister a disconnected agent.
    pub fn unregister(&self, agent_id: Uuid) {
        self.connections.remove(&agent_id);
        self.pending_models
            .retain(|_, (agent, _)| *agent != agent_id);
        tracing::info!(%agent_id, "agent disconnected from WebSocket");
    }

    /// Send a message to a specific agent. Returns false if not connected.
    pub fn send_to_agent(&self, agent_id: Uuid, msg: WsMessage) -> bool {
        if let Some(tx) = self.connections.get(&agent_id) {
            tx.send(msg).is_ok()
        } else {
            false
        }
    }

    /// Find any connected agent from a list of candidate agent_ids.
    /// Returns the first connected one.
    pub fn find_connected_agent(&self, agent_ids: &[Uuid]) -> Option<Uuid> {
        agent_ids
            .iter()
            .find(|id| self.connections.contains_key(id))
            .copied()
    }

    /// Register a model request bound to the selected worker.
    pub fn register_model_request(
        &self,
        request_id: String,
        agent_id: Uuid,
    ) -> oneshot::Receiver<ModelReply> {
        let (tx, rx) = oneshot::channel();
        self.pending_models.insert(request_id, (agent_id, tx));
        rx
    }

    pub fn remove_model_request(&self, request_id: &str) {
        self.pending_models.remove(request_id);
    }

    pub fn complete_model_request(
        &self,
        agent_id: Uuid,
        request_id: &str,
        catalog: Option<diraigent_types::ChatModelCatalog>,
    ) {
        if let Some((_, (_, tx))) = self
            .pending_models
            .remove_if(request_id, |_, (agent, _)| *agent == agent_id)
        {
            // Never forward worker diagnostics (which could contain credentials).
            let _ = tx.send(catalog.ok_or_else(|| {
                "Model discovery failed. You can still enter a model manually.".into()
            }));
        }
    }

    /// Register a pending git request. Returns a receiver for the response.
    pub fn register_git_request(
        &self,
        request_id: String,
    ) -> oneshot::Receiver<GitResponsePayload> {
        let (tx, rx) = oneshot::channel();
        self.pending_git.insert(request_id, tx);
        rx
    }

    /// Complete a pending git request with a response.
    pub fn complete_git_request(&self, request_id: &str, response: GitResponsePayload) {
        if let Some((_, tx)) = self.pending_git.remove(request_id) {
            let _ = tx.send(response);
        }
    }

    // ── Chat sessions ──

    /// Register an active chat session and the agent handling it.
    pub fn register_chat_session(
        &self,
        session_id: String,
        tx: mpsc::Sender<ChatSseEvent>,
        agent_id: Uuid,
    ) {
        self.session_agents.insert(session_id.clone(), agent_id);
        self.active_chats.insert(
            session_id,
            ActiveChat {
                tx,
                last_activity: tokio::time::Instant::now(),
            },
        );
    }

    /// Route a chat event to the correct session.
    pub async fn route_chat_event(&self, agent_id: Uuid, session_id: &str, event: ChatSseEvent) {
        if self
            .session_agents
            .get(session_id)
            .is_none_or(|owner| *owner != agent_id)
        {
            return;
        }
        let is_terminal = matches!(
            &event,
            ChatSseEvent::Done { .. } | ChatSseEvent::Error { .. }
        );
        // Do not hold a DashMap lock across a backpressured SSE send.
        let tx = self.active_chats.get_mut(session_id).map(|mut chat| {
            chat.last_activity = tokio::time::Instant::now();
            chat.tx.clone()
        });
        if is_terminal {
            self.remove_chat_session(session_id);
        }
        if let Some(tx) = tx {
            let _ = tx.send(event).await;
        }
    }

    pub fn chat_idle_deadline(
        &self,
        session_id: &str,
        timeout: std::time::Duration,
    ) -> Option<tokio::time::Instant> {
        self.active_chats
            .get(session_id)
            .map(|chat| chat.last_activity + timeout)
    }

    /// Atomically recheck activity before cancellation: a worker event arriving
    /// at the deadline must not lose its session to a stale timer.
    pub fn expire_idle_chat(&self, session_id: &str, timeout: std::time::Duration) -> bool {
        let removed = self.active_chats.remove_if(session_id, |_, chat| {
            tokio::time::Instant::now().duration_since(chat.last_activity) >= timeout
        });
        let Some((_, chat)) = removed else {
            return false;
        };
        let _ = chat.tx.try_send(ChatSseEvent::Error {
            message: "Chat session timed out after 10 minutes without worker activity".into(),
        });
        self.cancel_chat_session(session_id);
        true
    }

    /// Check if a chat session is still active.
    pub fn is_chat_active(&self, session_id: &str) -> bool {
        self.active_chats.contains_key(session_id)
    }

    /// Remove a chat session (e.g. on timeout).
    pub fn remove_chat_session(&self, session_id: &str) {
        self.active_chats.remove(session_id);
        self.session_agents.remove(session_id);
    }

    /// Check if the SSE receiver has been dropped (client disconnected).
    /// Returns true if the sender's receiver is closed.
    pub fn is_chat_sender_closed(&self, session_id: &str) -> bool {
        if let Some(tx) = self.active_chats.get(session_id) {
            tx.tx.is_closed()
        } else {
            true // session doesn't exist = effectively closed
        }
    }

    /// Cancel an active chat session by sending a ChatCancel message to the
    /// orchestra agent that is handling it. Returns true if the cancel was sent.
    pub fn cancel_chat_session(&self, session_id: &str) -> bool {
        if let Some(agent_id) = self.session_agents.get(session_id).map(|r| *r) {
            let msg = WsMessage::ChatCancel {
                session_id: session_id.to_string(),
            };
            let sent = self.send_to_agent(agent_id, msg);
            self.active_chats.remove(session_id);
            self.session_agents.remove(session_id);
            if sent {
                tracing::info!(session_id, %agent_id, "sent chat cancel to orchestra");
            }
            sent
        } else {
            false
        }
    }

    /// Check if any agent is connected.
    pub fn has_connections(&self) -> bool {
        !self.connections.is_empty()
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;

    fn catalog() -> diraigent_types::ChatModelCatalog {
        diraigent_types::ChatModelCatalog {
            provider: "opencode".into(),
            default_model: None,
            models: vec!["custom/model".into()],
        }
    }

    #[tokio::test]
    async fn model_reply_must_come_from_the_selected_worker() {
        let registry = WsRegistry::new();
        let worker = Uuid::new_v4();
        let mut rx = registry.register_model_request("request".into(), worker);
        registry.complete_model_request(Uuid::new_v4(), "request", Some(catalog()));
        assert!(matches!(
            rx.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        registry.complete_model_request(worker, "request", Some(catalog()));
        assert_eq!(rx.await.unwrap().unwrap().models, vec!["custom/model"]);
        assert!(registry.pending_models.is_empty());
    }

    #[tokio::test]
    async fn disconnect_and_cancellation_remove_pending_catalogs() {
        let registry = WsRegistry::new();
        let worker = Uuid::new_v4();
        let rx = registry.register_model_request("disconnect".into(), worker);
        let other = Uuid::new_v4();
        let other_rx = registry.register_model_request("cancel".into(), other);
        registry.unregister(worker);
        assert!(rx.await.is_err());
        assert_eq!(registry.pending_models.len(), 1);
        registry.remove_model_request("cancel");
        assert!(other_rx.await.is_err());
        registry.complete_model_request(other, "cancel", Some(catalog()));
        assert!(registry.pending_models.is_empty());
    }
}

#[cfg(test)]
mod chat_idle_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn worker_events_extend_only_their_own_idle_deadline() {
        let registry = WsRegistry::new();
        let agent = Uuid::new_v4();
        let (tx, mut rx) = mpsc::channel(8);
        let timeout = Duration::from_secs(600);
        registry.register_chat_session("active".into(), tx.clone(), agent);
        registry.register_chat_session("idle".into(), tx, agent);
        let old = tokio::time::Instant::now() - Duration::from_secs(601);
        registry
            .active_chats
            .get_mut("active")
            .unwrap()
            .last_activity = old;
        registry.active_chats.get_mut("idle").unwrap().last_activity = old;
        registry
            .route_chat_event(
                Uuid::new_v4(),
                "idle",
                ChatSseEvent::Text {
                    content: "wrong worker".into(),
                },
            )
            .await;
        assert_eq!(
            registry.active_chats.get("idle").unwrap().last_activity,
            old
        );
        registry
            .route_chat_event(
                agent,
                "active",
                ChatSseEvent::Text {
                    content: "progress".into(),
                },
            )
            .await;
        assert!(!registry.expire_idle_chat("active", timeout));
        assert!(registry.expire_idle_chat("idle", timeout));
        assert!(registry.is_chat_active("active"));
        assert!(!registry.is_chat_active("idle"));
        assert!(matches!(rx.recv().await, Some(ChatSseEvent::Text { .. })));
        assert!(matches!(rx.recv().await, Some(ChatSseEvent::Error { .. })));
        assert!(!registry.session_agents.contains_key("idle"));
    }

    #[tokio::test]
    async fn idle_timeout_cancels_worker_once_and_terminal_events_clean_up() {
        let registry = WsRegistry::new();
        let agent = Uuid::new_v4();
        let (worker_tx, mut worker_rx) = mpsc::unbounded_channel();
        registry.register(agent, worker_tx);
        let (tx, mut rx) = mpsc::channel(8);
        registry.register_chat_session("idle".into(), tx.clone(), agent);
        registry.active_chats.get_mut("idle").unwrap().last_activity =
            tokio::time::Instant::now() - Duration::from_secs(600);
        assert!(registry.expire_idle_chat("idle", Duration::from_secs(600)));
        assert!(!registry.expire_idle_chat("idle", Duration::from_secs(600)));
        assert!(
            matches!(worker_rx.try_recv(), Ok(WsMessage::ChatCancel { session_id }) if session_id == "idle")
        );
        assert!(worker_rx.try_recv().is_err());
        assert!(matches!(rx.recv().await, Some(ChatSseEvent::Error { .. })));
        registry.register_chat_session("done".into(), tx, agent);
        registry
            .route_chat_event(
                agent,
                "done",
                ChatSseEvent::Error {
                    message: "finished".into(),
                },
            )
            .await;
        assert!(
            registry
                .chat_idle_deadline("done", Duration::from_secs(600))
                .is_none()
        );
        assert!(!registry.session_agents.contains_key("done"));
        assert!(!registry.expire_idle_chat("done", Duration::ZERO));
    }
}
