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

/// Payload returned for completed plan requests.
pub struct PlanResponsePayload {
    pub success: bool,
    pub error: Option<String>,
    pub tasks: serde_json::Value,
}

pub struct WsRegistry {
    /// Connected orchestras: agent_id -> WS sender
    connections: DashMap<Uuid, mpsc::UnboundedSender<WsMessage>>,
    /// Pending git requests: request_id -> oneshot sender
    pending_git: DashMap<String, oneshot::Sender<GitResponsePayload>>,
    /// Pending plan requests: request_id -> oneshot sender
    pending_plan: DashMap<String, oneshot::Sender<PlanResponsePayload>>,
    /// Active chat sessions: session_id -> mpsc sender for SSE events
    active_chats: DashMap<String, mpsc::Sender<ChatSseEvent>>,
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
            pending_plan: DashMap::new(),
            active_chats: DashMap::new(),
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

    /// Register a pending plan request. Returns a receiver for the response.
    pub fn register_plan_request(
        &self,
        request_id: String,
    ) -> oneshot::Receiver<PlanResponsePayload> {
        let (tx, rx) = oneshot::channel();
        self.pending_plan.insert(request_id, tx);
        rx
    }

    /// Complete a pending plan request with a response.
    pub fn complete_plan_request(&self, request_id: &str, response: PlanResponsePayload) {
        if let Some((_, tx)) = self.pending_plan.remove(request_id) {
            let _ = tx.send(response);
        }
    }

    /// Register an active chat session.
    pub fn register_chat_session(&self, session_id: String, tx: mpsc::Sender<ChatSseEvent>) {
        self.active_chats.insert(session_id, tx);
    }

    /// Route a chat event to the correct session.
    pub async fn route_chat_event(&self, session_id: &str, event: ChatSseEvent) {
        let is_terminal = matches!(
            &event,
            ChatSseEvent::Done { .. } | ChatSseEvent::Error { .. }
        );
        if let Some(tx) = self.active_chats.get(session_id) {
            let _ = tx.send(event).await;
        }
        if is_terminal {
            self.active_chats.remove(session_id);
        }
    }

    /// Check if a chat session is still active.
    pub fn is_chat_active(&self, session_id: &str) -> bool {
        self.active_chats.contains_key(session_id)
    }

    /// Remove a chat session (e.g. on timeout).
    pub fn remove_chat_session(&self, session_id: &str) {
        self.active_chats.remove(session_id);
    }

    /// Check if any agent is connected.
    pub fn has_connections(&self) -> bool {
        !self.connections.is_empty()
    }
}
