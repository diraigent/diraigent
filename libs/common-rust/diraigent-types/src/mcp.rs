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
