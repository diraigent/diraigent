use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chat::{ChatSseEvent, Message};

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
        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        remote: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
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
