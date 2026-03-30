use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use diraigent_types::{ChatSseEvent, DoneMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    // API -> Orchestra
    #[serde(rename = "chat.request")]
    ChatRequest {
        session_id: String,
        project_id: Uuid,
        user_id: Uuid,
        messages: Vec<Message>,
        system_prompt: String,
        model: String,
    },
    #[serde(rename = "git.request")]
    GitRequest {
        request_id: String,
        project_id: Uuid,
        query_type: String,
        prefix: Option<String>,
        task_id: Option<String>,
        branch: Option<String>,
        remote: Option<String>,
        path: Option<String>,
        git_ref: Option<String>,
    },

    // API -> Orchestra: cancel an active chat session
    #[serde(rename = "chat.cancel")]
    ChatCancel { session_id: String },

    // Orchestra -> API
    #[serde(rename = "chat.event")]
    ChatEvent {
        session_id: String,
        event: ChatSseEvent,
    },
    #[serde(rename = "git.response")]
    GitResponse {
        request_id: String,
        success: bool,
        error: Option<String>,
        data: serde_json::Value,
    },
    #[serde(rename = "heartbeat")]
    Heartbeat,
}
