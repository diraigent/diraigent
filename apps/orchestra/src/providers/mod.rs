//! Provider abstraction for step execution.
//!
//! This module defines the [`StepProvider`] trait and a [`ProviderFactory`] for
//! creating provider instances by name.  Three providers are registered out of
//! the box: `anthropic`, `openai`, and `ollama`.

mod anthropic;
mod ollama;
mod openai;

use std::collections::HashMap;

use async_trait::async_trait;

// ── Shared types ────────────────────────────────────────────────────────────

/// A playbook step with all template variables substituted and fields resolved.
#[derive(Debug, Clone)]
pub struct ResolvedStep {
    /// Step name (e.g. "implement", "review").
    pub name: String,
    /// Fully-resolved description/prompt for the step.
    pub description: String,
    /// Model to use (e.g. "claude-sonnet-4-6", "gpt-4o", "llama3").
    pub model: Option<String>,
    /// Tool preset: "full", "readonly", "merge".
    pub allowed_tools: Option<String>,
    /// Maximum budget in USD for this step.
    pub budget: Option<f64>,
    /// Extra environment variables for the step.
    pub env: HashMap<String, String>,
}

/// Context about the task being executed.
#[derive(Debug, Clone)]
pub struct TaskContext {
    /// The task's UUID.
    pub task_id: String,
    /// The project's UUID.
    pub project_id: String,
    /// Serialised project context (JSON string).
    pub project_context: String,
    /// Output from the previous step, if any.
    pub previous_step_output: Option<String>,
}

/// Credentials and endpoint configuration for a provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// API key / bearer token for the provider.
    pub api_key: Option<String>,
    /// Base URL override (e.g. `https://api.openai.com`).
    pub base_url: Option<String>,
    /// Default model for this provider.
    pub model: Option<String>,
}

/// The output produced by a provider after executing a step.
#[derive(Debug, Clone)]
pub struct StepOutput {
    /// The textual content returned by the provider.
    pub content: String,
    /// Process exit code (0 = success).
    pub exit_code: i32,
    /// Optional key-value artifacts produced during execution.
    pub artifacts: HashMap<String, String>,
}

// ── Trait ────────────────────────────────────────────────────────────────────

/// Trait for executing a playbook step via a specific LLM provider.
///
/// Implementors handle the details of calling the provider's API or spawning
/// a subprocess (as in the Anthropic/Claude Code case).
#[async_trait]
pub trait StepProvider: Send + Sync {
    /// Execute the given step in the context of the given task.
    async fn execute(
        &self,
        step: &ResolvedStep,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<StepOutput>;
}

// ── Factory ─────────────────────────────────────────────────────────────────

/// Error returned when an unknown provider name is requested.
#[derive(Debug, thiserror::Error)]
#[error("unknown provider: \"{0}\"")]
pub struct UnknownProviderError(String);

/// Factory that creates [`StepProvider`] instances by provider name.
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a boxed [`StepProvider`] for the given provider name.
    ///
    /// Known providers: `"anthropic"`, `"openai"`, `"ollama"`.
    ///
    /// Returns [`UnknownProviderError`] for any unrecognised name.
    pub fn create(provider_name: &str) -> Result<Box<dyn StepProvider>, UnknownProviderError> {
        match provider_name {
            "anthropic" => Ok(Box::new(anthropic::AnthropicProvider)),
            "openai" => Ok(Box::new(openai::OpenAIProvider)),
            "ollama" => Ok(Box::new(ollama::OllamaProvider)),
            other => Err(UnknownProviderError(other.to_string())),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_creates_anthropic() {
        let provider = ProviderFactory::create("anthropic");
        assert!(provider.is_ok(), "anthropic provider should be created");
    }

    #[test]
    fn factory_creates_openai() {
        let provider = ProviderFactory::create("openai");
        assert!(provider.is_ok(), "openai provider should be created");
    }

    #[test]
    fn factory_creates_ollama() {
        let provider = ProviderFactory::create("ollama");
        assert!(provider.is_ok(), "ollama provider should be created");
    }

    #[test]
    fn factory_rejects_unknown_provider() {
        let result = ProviderFactory::create("foobar");
        assert!(result.is_err(), "unknown provider should return error");
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "unknown provider: \"foobar\"");
    }

    #[test]
    fn factory_rejects_empty_provider() {
        let result = ProviderFactory::create("");
        assert!(result.is_err(), "empty provider name should return error");
    }

    #[tokio::test]
    async fn anthropic_provider_implements_trait() {
        let provider = ProviderFactory::create("anthropic").unwrap();
        let step = ResolvedStep {
            name: "test".into(),
            description: "test step".into(),
            model: None,
            allowed_tools: None,
            budget: None,
            env: HashMap::new(),
        };
        let task = TaskContext {
            task_id: "test-task-id".into(),
            project_id: "test-project-id".into(),
            project_context: "{}".into(),
            previous_step_output: None,
        };
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        // The stub implementation returns a placeholder — just verify it doesn't panic.
        let result = provider.execute(&step, &task, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn openai_provider_implements_trait() {
        let provider = ProviderFactory::create("openai").unwrap();
        let step = ResolvedStep {
            name: "test".into(),
            description: "test step".into(),
            model: None,
            allowed_tools: None,
            budget: None,
            env: HashMap::new(),
        };
        let task = TaskContext {
            task_id: "test-task-id".into(),
            project_id: "test-project-id".into(),
            project_context: "{}".into(),
            previous_step_output: None,
        };
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let result = provider.execute(&step, &task, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn ollama_provider_implements_trait() {
        let provider = ProviderFactory::create("ollama").unwrap();
        let step = ResolvedStep {
            name: "test".into(),
            description: "test step".into(),
            model: None,
            allowed_tools: None,
            budget: None,
            env: HashMap::new(),
        };
        let task = TaskContext {
            task_id: "test-task-id".into(),
            project_id: "test-project-id".into(),
            project_context: "{}".into(),
            previous_step_output: None,
        };
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let result = provider.execute(&step, &task, &config).await;
        assert!(result.is_ok());
    }
}
