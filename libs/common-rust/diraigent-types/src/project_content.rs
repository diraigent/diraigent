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
    Field,
}
impl ContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Diff => "diff",
            Self::Artifact => "artifact",
            Self::Field => "field",
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
    SearchFields {
        candidates: Vec<ContentSearchCandidate>,
        query: String,
        limit: usize,
        offset: usize,
    },
    RankFields {
        candidates: Vec<(Uuid, FieldRef)>,
        vector: Vec<f64>,
        limit: usize,
    },
}

/// Searchable metadata stays central; referenced field bodies are searched locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSearchCandidate {
    pub entity_type: String,
    pub id: Uuid,
    pub title: String,
    pub created_at: String,
    pub fields: Vec<Value>,
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

/// Immutable, content-address-verified reference. String columns encode the
/// same reference as JSON with a reserved prefix; JSON columns retain controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldRef {
    pub project: Uuid,
    pub id: Uuid,
    pub checksum: String,
    pub bytes: usize,
}
pub const FIELD_KEY: &str = "__orchestra_field_v1";
pub const FIELD_PREFIX: &str = "orchestra-field:v1:";
impl FieldRef {
    pub fn parse(value: &Value) -> Result<Option<Self>, serde_json::Error> {
        if let Some(s) = value.as_str().and_then(|s| s.strip_prefix(FIELD_PREFIX)) {
            return serde_json::from_str(s).map(Some);
        }
        if let Some(items) = value.as_array()
            && items.len() == 1
            && let Some(name) = items[0].get("name")
            && name.as_str().is_some_and(|s| s.starts_with(FIELD_PREFIX))
        {
            return Self::parse(name);
        }
        value
            .get(FIELD_KEY)
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
    }
    pub fn encode(&self, original: &Value) -> Value {
        if original.is_string() {
            return Value::String(format!(
                "{FIELD_PREFIX}{}",
                serde_json::to_string(self).expect("field reference")
            ));
        }
        let mut marker = serde_json::json!({FIELD_KEY:self});
        // Task routing controls must remain available to SQL and schedulers.
        if let Some(worker) = original.get("worker") {
            marker["worker"] = worker.clone();
        }
        marker
    }
}
