use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(error: &str, message: &str) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fid {
    pub source: String,
    pub tag: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct Zuuid {
    pub zuuid: Uuid,
}

#[derive(Debug, Serialize)]
pub struct ListFids {
    pub fids: Vec<Fid>,
}

#[derive(Debug, Serialize)]
pub struct ListZuuids {
    pub zuuids: Vec<Uuid>,
}

pub trait Validate {
    fn validate_non_empty(&self, field_name: &str) -> Result<&Self>;
}

impl Validate for String {
    fn validate_non_empty(&self, field_name: &str) -> Result<&Self> {
        if self.trim().is_empty() {
            bail!("{} cannot be empty", field_name);
        }
        Ok(self)
    }
}

impl Validate for str {
    fn validate_non_empty(&self, field_name: &str) -> Result<&Self> {
        if self.trim().is_empty() {
            bail!("{} cannot be empty", field_name);
        }
        Ok(self)
    }
}
