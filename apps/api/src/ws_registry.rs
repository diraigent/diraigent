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

/// Payload returned for completed playbook requests.
pub struct PlaybookResponsePayload {
    pub success: bool,
    pub error: Option<String>,
    pub data: serde_json::Value,
}

pub struct WsRegistry {
    /// Connected orchestras: agent_id -> WS sender
    connections: DashMap<Uuid, mpsc::UnboundedSender<WsMessage>>,
    /// Pending git requests: request_id -> oneshot sender
    pending_git: DashMap<String, oneshot::Sender<GitResponsePayload>>,
    /// Pending playbook requests: request_id -> oneshot sender
    pending_playbook: DashMap<String, oneshot::Sender<PlaybookResponsePayload>>,
    /// Active chat sessions: session_id -> mpsc sender for SSE events
    active_chats: DashMap<String, mpsc::Sender<ChatSseEvent>>,
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
            pending_playbook: DashMap::new(),
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

    /// Register a pending playbook request. Returns a receiver for the response.
    pub fn register_playbook_request(
        &self,
        request_id: String,
    ) -> oneshot::Receiver<PlaybookResponsePayload> {
        let (tx, rx) = oneshot::channel();
        self.pending_playbook.insert(request_id, tx);
        rx
    }

    /// Complete a pending playbook request with a response.
    pub fn complete_playbook_request(
        &self,
        request_id: &str,
        response: PlaybookResponsePayload,
    ) {
        if let Some((_, tx)) = self.pending_playbook.remove(request_id) {
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
            self.session_agents.remove(session_id);
        }
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
            tx.is_closed()
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
