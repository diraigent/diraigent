//! Anthropic provider — wraps the existing Claude Code subprocess invocation.
//!
//! This is a stub implementation.  The real logic (spawning `claude -p` with
//! the correct flags, environment, and PTY wrapper) currently lives in
//! [`crate::worker::run_claude`] and will be migrated here in a follow-up task.

use async_trait::async_trait;

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

/// Provider that executes steps via Anthropic's Claude Code CLI.
pub struct AnthropicProvider;

#[async_trait]
impl StepProvider for AnthropicProvider {
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
