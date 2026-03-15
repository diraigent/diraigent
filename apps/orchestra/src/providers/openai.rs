//! OpenAI provider — calls the OpenAI-compatible chat completions API.
//!
//! This is a stub implementation.  The real logic (HTTP client, streaming
//! response parsing, error mapping) will be added in a follow-up task.

use async_trait::async_trait;

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

/// Provider that executes steps via OpenAI-compatible APIs.
pub struct OpenAIProvider;

#[async_trait]
impl StepProvider for OpenAIProvider {
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
