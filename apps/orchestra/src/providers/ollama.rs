//! Ollama provider — calls the local Ollama chat API.
//!
//! This is a stub implementation.  The real logic (HTTP client, NDJSON
//! streaming, error handling for connection refused / model not found)
//! will be added in a follow-up task.

use async_trait::async_trait;

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

/// Provider that executes steps via a local Ollama instance.
pub struct OllamaProvider;

#[async_trait]
impl StepProvider for OllamaProvider {
    async fn execute(
        &self,
        _step: &ResolvedStep,
        _task: &TaskContext,
        _config: &ProviderConfig,
    ) -> anyhow::Result<StepOutput> {
        // Stub — full implementation in a follow-up task.
        Ok(StepOutput {
            content: String::new(),
            exit_code: 0,
            artifacts: Default::default(),
        })
    }
}
