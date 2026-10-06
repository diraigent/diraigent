//! Version 1 of the approved project MCP configuration contract.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "transport", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpTransport {
    Stdio {
        executable: String,
        #[serde(default)]
        arguments: Vec<String>,
    },
    StreamableHttp {
        endpoint: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum McpToolAccess {
    Read,
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct McpToolPermission {
    pub name: String,
    pub access: McpToolAccess,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct McpConfiguration {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default)]
    pub tools: Vec<McpToolPermission>,
    /// Environment variable or HTTP header name -> credential key (never a value).
    #[serde(default)]
    pub credential_bindings: BTreeMap<String, String>,
}

/// Sanitized bridge payloads: never include upstream errors, URLs or secrets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpCheckResult {
    pub server_id: uuid::Uuid,
    pub revision: i64,
    pub status: McpCheckStatus,
    pub approved_tools: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpCheckStatus {
    Connected,
    Denied,
    Failed,
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpAuditEvent {
    pub project_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub server_id: uuid::Uuid,
    pub revision: i64,
    pub tool_name: Option<String>,
    pub outcome: McpAuditOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuditOutcome {
    Succeeded,
    Denied,
    Failed,
    TimedOut,
    Cancelled,
}
