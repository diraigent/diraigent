//! Provider abstraction for task execution.
//!
//! This module defines the [`TaskProvider`] trait and a [`ProviderFactory`] for
//! creating provider instances by name. Seven providers are registered:
//!
//! - `claude-code` — Claude Code CLI subprocess (agentic, PTY, tools)
//! - `codex` — Codex CLI subprocess (agentic, sandboxed)
//! - `opencode` — OpenCode CLI subprocess (agentic, default)
//! - `anthropic` — Anthropic Messages API (direct, non-streaming)
//! - `openai` — OpenAI-compatible chat completions API (SSE streaming)
//! - `copilot` — GitHub Copilot / GitHub Models inference API (OpenAI-compatible, SSE streaming)
//! - `ollama` — local Ollama chat API (NDJSON streaming)

mod anthropic;
mod claude_code;
mod codex;
mod copilot;
mod ollama;
mod openai;
mod opencode;
pub(crate) use opencode::run_command as opencode_run_command;

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

// ── Shared types ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ResolvedTask {
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
    /// Opaque task-local broker connections, never raw provider configuration.
    pub mcp_servers: Option<crate::engine::mcp::Sessions>,
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

/// The output produced by a provider after executing a task.
#[derive(Debug, Clone)]
pub struct TaskOutput {
    /// The textual content returned by the provider.
    pub content: String,
    /// Process exit code (0 = success).
    pub exit_code: i32,
    /// Optional key-value artifacts produced during execution.
    pub artifacts: HashMap<String, String>,
    /// Total cost in USD for task execution.
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

///
/// Implementors handle the details of calling the provider's API or spawning
/// a subprocess (as in the Claude Code case).
#[async_trait]
pub trait TaskProvider: Send + Sync {
    /// Execute the given step in the context of the given task.
    async fn execute(
        &self,
        step: &ResolvedTask,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<TaskOutput>;
}

// ── Factory ─────────────────────────────────────────────────────────────────

/// Error returned when an unknown provider name is requested.
#[derive(Debug, thiserror::Error)]
#[error("unknown provider: \"{0}\"")]
pub struct UnknownProviderError(String);

/// Factory that creates [`TaskProvider`] instances by provider name.
pub struct ProviderFactory;

#[derive(Debug, PartialEq, Eq)]
pub enum McpCapability {
    BrokerAdapter,
    Unsupported,
}

struct GuardedProvider {
    name: String,
    inner: Box<dyn TaskProvider>,
}
#[async_trait]
impl TaskProvider for GuardedProvider {
    async fn execute(
        &self,
        step: &ResolvedTask,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<TaskOutput> {
        if step.mcp_servers.as_ref().is_some_and(|s| !s.0.is_empty()) {
            ProviderFactory::require_mcp(&self.name)?;
            anyhow::bail!(
                "Provider '{}' MCP broker adapter is not implemented yet; refusing to ignore approved MCP access",
                self.name
            );
        }
        self.inner.execute(step, task, config).await
    }
}

impl ProviderFactory {
    pub fn mcp_capability(name: &str) -> McpCapability {
        match name {
            "claude-code" | "codex" | "opencode" => McpCapability::BrokerAdapter,
            _ => McpCapability::Unsupported,
        }
    }
    pub fn require_mcp(name: &str) -> anyhow::Result<()> {
        if Self::mcp_capability(name) == McpCapability::Unsupported {
            anyhow::bail!(
                "Provider '{name}' does not support MCP; select claude-code, codex or opencode with a broker adapter"
            );
        }
        Ok(())
    }
    /// Create a boxed [`TaskProvider`] for the given provider name.
    ///
    /// Known providers: `"opencode"`, `"claude-code"`, `"codex"`, `"anthropic"`, `"openai"`, `"copilot"`, `"ollama"`.
    ///
    /// Returns [`UnknownProviderError`] for any unrecognised name.
    pub fn create(provider_name: &str) -> Result<Box<dyn TaskProvider>, UnknownProviderError> {
        let inner: Box<dyn TaskProvider> = match provider_name {
            "opencode" => Box::new(opencode::OpenCodeProvider),
            "claude-code" => Box::new(claude_code::ClaudeCodeProvider),
            "codex" => Box::new(codex::CodexProvider),
            "anthropic" => Box::new(anthropic::AnthropicProvider),
            "openai" => Box::new(openai::OpenAIProvider),
            "copilot" => Box::new(copilot::CopilotProvider),
            "ollama" => Box::new(ollama::OllamaProvider),
            other => return Err(UnknownProviderError(other.to_string())),
        };
        Ok(Box::new(GuardedProvider {
            name: provider_name.into(),
            inner,
        }))
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
    fn factory_creates_codex() {
        assert!(ProviderFactory::create("codex").is_ok());
    }

    #[test]
    fn factory_creates_opencode() {
        assert!(ProviderFactory::create("opencode").is_ok());
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

    fn test_step() -> ResolvedTask {
        ResolvedTask {
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
