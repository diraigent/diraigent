//! Project payload protocol. Identity and permissions are checked by the API;
//! the selected Orchestra owns durable content, never an arbitrary worker.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Log,
    Diff,
    Artifact,
}
impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Diff => "diff",
            Self::Artifact => "artifact",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistory {
    pub enabled: bool,
    pub revision: i64,
    pub messages: Vec<HistoryMessage>,
    pub busy: bool,
}
impl Default for ChatHistory {
    fn default() -> Self {
        Self {
            enabled: true,
            revision: 0,
            messages: Vec::new(),
            busy: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ContentRequest {
    Probe,
    Get {
        kind: ContentKind,
        id: Uuid,
    },
    Put {
        kind: ContentKind,
        id: Uuid,
        value: Value,
    },
    History {
        user_id: Uuid,
    },
    ClearHistory {
        user_id: Uuid,
        revision: i64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentError {
    NotFound,
    Conflict,
    Invalid,
    Unavailable,
}
pub type ContentResult = Result<Value, ContentError>;
