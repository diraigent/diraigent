//! Provider abstraction for step execution.
//!
//! This module defines the [`StepProvider`] trait and a [`ProviderFactory`] for
//! creating provider instances by name.  Five providers are registered:
//!
//! - `claude-code` — Claude Code CLI subprocess (agentic, PTY, tools)
//! - `anthropic` — Anthropic Messages API (direct, non-streaming)
//! - `openai` — OpenAI-compatible chat completions API (SSE streaming)
//! - `copilot` — GitHub Copilot / GitHub Models inference API (OpenAI-compatible, SSE streaming)
//! - `ollama` — local Ollama chat API (NDJSON streaming)

mod anthropic;
mod claude_code;
mod copilot;
mod ollama;
mod openai;

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

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
    /// Resolved list of allowed tool names (e.g. `["Bash(*)", "Read", ...]`).
    pub allowed_tools_list: Vec<String>,
    /// Maximum budget in USD for this step.
    pub budget: Option<f64>,
    /// Extra environment variables for the step.
    pub env: HashMap<String, String>,
    /// System prompt (static CLAUDE.md-based prompt, used by Claude Code provider).
    pub system_prompt: Option<String>,
    /// MCP server configurations (JSON object with `"mcpServers"` key).
    pub mcp_servers: Option<Value>,
    /// Custom sub-agent definitions (JSON object, key=name, value={description, prompt}).
    pub agents: Option<Value>,
    /// Name of a configured agent to activate.
    pub agent: Option<String>,
    /// Additional Claude settings (skills, keybindings, etc.) as JSON.
    pub settings: Option<Value>,
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
    /// Working directory (git worktree path). Required by Claude Code provider.
    pub working_dir: Option<PathBuf>,
    /// Log file path for PTY recording. Required by Claude Code provider.
    pub log_file: Option<PathBuf>,
    /// Direct user prompt. When set, providers use this as the user message
    /// content instead of building a JSON envelope from the other fields.
    /// Used by plan_handler and chat summarization.
    pub user_prompt: Option<String>,
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
    /// Total cost in USD for the step execution.
    pub cost_usd: f64,
    /// Number of input tokens consumed.
    pub input_tokens: u64,
    /// Number of output tokens produced.
    pub output_tokens: u64,
    /// Number of API turns (conversation round-trips).
    pub num_turns: u64,
    /// Reason the provider stopped (e.g. "end_turn", "max_tokens").
    pub stop_reason: String,
    /// Whether the provider flagged this execution as an error.
    pub is_error: bool,
}

// ── Trait ────────────────────────────────────────────────────────────────────

/// Trait for executing a playbook step via a specific LLM provider.
///
/// Implementors handle the details of calling the provider's API or spawning
/// a subprocess (as in the Claude Code case).
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
    /// Known providers: `"claude-code"`, `"anthropic"`, `"openai"`, `"copilot"`, `"ollama"`.
    ///
    /// Returns [`UnknownProviderError`] for any unrecognised name.
    pub fn create(provider_name: &str) -> Result<Box<dyn StepProvider>, UnknownProviderError> {
        match provider_name {
            "claude-code" => Ok(Box::new(claude_code::ClaudeCodeProvider)),
            "anthropic" => Ok(Box::new(anthropic::AnthropicProvider)),
            "openai" => Ok(Box::new(openai::OpenAIProvider)),
            "copilot" => Ok(Box::new(copilot::CopilotProvider)),
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
    fn factory_creates_claude_code() {
        let provider = ProviderFactory::create("claude-code");
        assert!(provider.is_ok(), "claude-code provider should be created");
    }

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
    fn factory_creates_copilot() {
        let provider = ProviderFactory::create("copilot");
        assert!(provider.is_ok(), "copilot provider should be created");
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

    fn test_step() -> ResolvedStep {
        ResolvedStep {
            name: "test".into(),
            description: "test step".into(),
            model: None,
            allowed_tools: None,
            allowed_tools_list: vec![],
            budget: None,
            env: HashMap::new(),
            system_prompt: None,
            mcp_servers: None,
            agents: None,
            agent: None,
            settings: None,
        }
    }

    fn test_task() -> TaskContext {
        TaskContext {
            task_id: "test-task-id".into(),
            project_id: "test-project-id".into(),
            project_context: "{}".into(),
            previous_step_output: None,
            working_dir: None,
            log_file: None,
            user_prompt: None,
        }
    }

    #[tokio::test]
    async fn claude_code_provider_requires_working_dir() {
        let provider = ProviderFactory::create("claude-code").unwrap();
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err(), "should require working_dir");
    }

    #[tokio::test]
    async fn anthropic_provider_requires_api_key() {
        let provider = ProviderFactory::create("anthropic").unwrap();
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err(), "missing API key should return error");
    }

    #[tokio::test]
    async fn openai_provider_requires_api_key() {
        let provider = ProviderFactory::create("openai").unwrap();
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err(), "missing API key should return error");
    }

    #[tokio::test]
    async fn copilot_provider_requires_token() {
        let provider = ProviderFactory::create("copilot").unwrap();
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err(), "missing token should return error");
    }

    #[tokio::test]
    async fn ollama_provider_implements_trait() {
        let provider = ProviderFactory::create("ollama").unwrap();
        let config = ProviderConfig {
            api_key: None,
            base_url: Some("http://127.0.0.1:19999".to_string()),
            model: None,
        };
        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err());
    }
}
