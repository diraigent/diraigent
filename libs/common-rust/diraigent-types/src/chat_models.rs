use serde::{Deserialize, Serialize};

/// Only displayable catalog data crosses the worker/API boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatModelCatalog {
    pub provider: String,
    pub default_model: Option<String>,
    pub models: Vec<String>,
}
