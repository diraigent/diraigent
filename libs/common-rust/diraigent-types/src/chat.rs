//! Shared chat types used by both the API and orchestra crates.

use serde::{Deserialize, Serialize};

/// Events streamed from the chat backend to the SSE client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatSseEvent {
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "thinking")]
    Thinking { content: String },
    #[serde(rename = "tool_start")]
    ToolStart { tool_name: String, tool_id: String },
    #[serde(rename = "tool_end")]
    ToolEnd { tool_id: String, success: bool },
    #[serde(rename = "done")]
    Done { message: DoneMessage },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Terminal message included in a [`ChatSseEvent::Done`] event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneMessage {
    pub role: String,
    pub content: String,
}
