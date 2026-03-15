//! Ollama provider — calls the local Ollama chat API.
//!
//! Sends a POST to `{base_url}/api/chat` with a JSON body containing the
//! `model` and `messages` fields.  Consumes Ollama's NDJSON streaming
//! response, accumulates content, and posts incremental progress updates.
//!
//! Error handling:
//! - Connection refused (Ollama not running) → [`OllamaError::ConnectionRefused`]
//! - HTTP 404 with model-not-found body → [`OllamaError::ModelNotFound`]

use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

// ── Default ────────────────────────────────────────────────────────────────

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "llama3";

// ── Error types ────────────────────────────────────────────────────────────

/// Typed errors specific to the Ollama provider.
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    /// Ollama server is not reachable (connection refused / timeout).
    #[error("connection refused: Ollama is not running at {url}")]
    ConnectionRefused { url: String },

    /// The requested model is not available on the Ollama instance.
    #[error("model not found: \"{model}\" is not available on {url}")]
    ModelNotFound { model: String, url: String },
}

// ── Request / Response types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// A single line of Ollama's NDJSON streaming response.
#[derive(Debug, Deserialize)]
struct ChatResponseChunk {
    /// The message fragment for this chunk.
    #[serde(default)]
    message: Option<ChunkMessage>,
    /// Whether this is the final chunk.
    #[serde(default)]
    done: bool,
    /// Error message, if any.
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkMessage {
    #[serde(default)]
    content: String,
}

// ── Provider ───────────────────────────────────────────────────────────────

/// Provider that executes steps via a local Ollama instance.
pub struct OllamaProvider;

impl OllamaProvider {
    /// Resolve the base URL from config, falling back to the default.
    fn base_url(config: &ProviderConfig) -> &str {
        config
            .base_url
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_BASE_URL)
    }

    /// Resolve the model from config, falling back to a sensible default.
    fn model(config: &ProviderConfig, step: &ResolvedStep) -> String {
        step.model
            .clone()
            .or_else(|| config.model.clone())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    }
}

#[async_trait]
impl StepProvider for OllamaProvider {
    async fn execute(
        &self,
        step: &ResolvedStep,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<StepOutput> {
        let base_url = Self::base_url(config);
        let model = Self::model(config, step);
        let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

        let request_body = ChatRequest {
            model: model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: step.description.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: task
                        .user_prompt
                        .clone()
                        .unwrap_or_else(|| task.project_context.clone()),
                },
            ],
            stream: true,
        };

        let client = Client::new();

        // Send the request — map connection errors to our typed error.
        let response = match client.post(&url).json(&request_body).send().await {
            Ok(resp) => resp,
            Err(e) if is_connect_error(&e) => {
                return Err(OllamaError::ConnectionRefused {
                    url: base_url.to_string(),
                }
                .into());
            }
            Err(e) => return Err(e.into()),
        };

        let status = response.status();

        // Handle HTTP 404 — Ollama returns 404 when the model is not found.
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(OllamaError::ModelNotFound {
                model,
                url: base_url.to_string(),
            }
            .into());
        }

        // Handle other non-success statuses.
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Ollama API returned HTTP {}: {}",
                status.as_u16(),
                body.chars().take(500).collect::<String>()
            );
        }

        // ── Stream NDJSON response ─────────────────────────────────────
        let mut accumulated = String::new();
        let mut chunk_count: usize = 0;

        // Read the body as bytes and process line by line.
        let body_bytes = response.bytes().await?;
        let body_text = String::from_utf8_lossy(&body_bytes);

        for line in body_text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let chunk: ChatResponseChunk = match serde_json::from_str(line) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(line, error = %e, "skipping unparseable NDJSON line");
                    continue;
                }
            };

            // Check for error in the chunk itself.
            if let Some(ref err_msg) = chunk.error {
                if err_msg.contains("not found") {
                    return Err(OllamaError::ModelNotFound {
                        model,
                        url: base_url.to_string(),
                    }
                    .into());
                }
                anyhow::bail!("Ollama API error: {}", err_msg);
            }

            // Accumulate content from the message fragment.
            if let Some(ref msg) = chunk.message
                && !msg.content.is_empty()
            {
                accumulated.push_str(&msg.content);
                chunk_count += 1;

                // Post incremental progress every 20 chunks.
                if chunk_count.is_multiple_of(20) {
                    tracing::debug!(
                        task_id = %task.task_id,
                        chunks = chunk_count,
                        chars = accumulated.len(),
                        "Ollama streaming progress"
                    );
                }
            }

            if chunk.done {
                break;
            }
        }

        tracing::info!(
            task_id = %task.task_id,
            model = %model,
            chunks = chunk_count,
            content_len = accumulated.len(),
            "Ollama step completed"
        );

        Ok(StepOutput {
            content: accumulated,
            exit_code: 0,
            artifacts: HashMap::new(),
            cost_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            num_turns: 0,
            stop_reason: "end_turn".into(),
            is_error: false,
        })
    }
}

/// Check whether a reqwest error is a connection error (connection refused,
/// DNS failure, timeout, etc.).
fn is_connect_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Helper: build a standard test step.
    fn test_step() -> ResolvedStep {
        ResolvedStep {
            name: "implement".into(),
            description: "Write some code".into(),
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

    /// Helper: build a standard test task context.
    fn test_task_context() -> TaskContext {
        TaskContext {
            task_id: "test-task-id".into(),
            project_id: "test-project-id".into(),
            project_context: r#"{"task": "do something"}"#.into(),
            previous_step_output: None,
            working_dir: None,
            log_file: None,
            user_prompt: None,
        }
    }

    /// Helper: build a provider config pointing at the given mock server.
    fn test_config(server_url: &str) -> ProviderConfig {
        ProviderConfig {
            api_key: None,
            base_url: Some(server_url.to_string()),
            model: Some("test-model".to_string()),
        }
    }

    /// Build an NDJSON body from a list of (content, done) pairs.
    fn ndjson_body(chunks: &[(&str, bool)]) -> String {
        chunks
            .iter()
            .map(|(content, done)| {
                serde_json::json!({
                    "message": { "role": "assistant", "content": content },
                    "done": done,
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ── Happy path ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_successful_streaming_response() {
        let server = MockServer::start().await;

        let body = ndjson_body(&[("Hello", false), (", ", false), ("world!", true)]);

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .insert_header("content-type", "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let provider = OllamaProvider;
        let config = test_config(&server.uri());
        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(result.is_ok(), "expected success, got: {:?}", result);
        let output = result.unwrap();
        assert_eq!(output.content, "Hello, world!");
        assert_eq!(output.exit_code, 0);
    }

    #[tokio::test]
    async fn test_request_body_contains_model_and_messages() {
        let server = MockServer::start().await;

        let body = ndjson_body(&[("ok", true)]);

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .and(body_partial_json(serde_json::json!({
                "model": "test-model",
                "stream": true,
                "messages": [
                    { "role": "system", "content": "Write some code" },
                    { "role": "user" }
                ]
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .insert_header("content-type", "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let provider = OllamaProvider;
        let config = test_config(&server.uri());
        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(
            result.is_ok(),
            "request body should match schema: {:?}",
            result
        );
    }

    // ── Connection refused ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_connection_refused() {
        let provider = OllamaProvider;
        // Use a port that nothing listens on.
        let config = ProviderConfig {
            api_key: None,
            base_url: Some("http://127.0.0.1:19999".to_string()),
            model: Some("test-model".to_string()),
        };

        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(result.is_err(), "expected error for connection refused");
        let err = result.unwrap_err();
        let ollama_err = err.downcast_ref::<OllamaError>();
        assert!(
            ollama_err.is_some(),
            "error should be OllamaError::ConnectionRefused, got: {err}"
        );
        match ollama_err.unwrap() {
            OllamaError::ConnectionRefused { .. } => {} // expected
            other => panic!("expected ConnectionRefused, got: {other}"),
        }
    }

    // ── Model not found (HTTP 404) ─────────────────────────────────────

    #[tokio::test]
    async fn test_model_not_found_http_404() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_string(r#"{"error":"model \"nonexistent\" not found"}"#),
            )
            .mount(&server)
            .await;

        let provider = OllamaProvider;
        let config = test_config(&server.uri());
        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(result.is_err(), "expected error for model not found");
        let err = result.unwrap_err();
        let ollama_err = err.downcast_ref::<OllamaError>();
        assert!(
            ollama_err.is_some(),
            "error should be OllamaError::ModelNotFound, got: {err}"
        );
        match ollama_err.unwrap() {
            OllamaError::ModelNotFound { model, .. } => {
                assert_eq!(model, "test-model");
            }
            other => panic!("expected ModelNotFound, got: {other}"),
        }
    }

    // ── Model not found (in-stream error) ──────────────────────────────

    #[tokio::test]
    async fn test_model_not_found_in_stream() {
        let server = MockServer::start().await;

        let body = r#"{"error":"model \"nonexistent\" not found","done":false}"#;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .insert_header("content-type", "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let provider = OllamaProvider;
        let config = test_config(&server.uri());
        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(
            result.is_err(),
            "expected error for model not found in stream"
        );
        let err = result.unwrap_err();
        let ollama_err = err.downcast_ref::<OllamaError>();
        assert!(
            ollama_err.is_some(),
            "error should be OllamaError::ModelNotFound, got: {err}"
        );
    }

    // ── Default base_url ───────────────────────────────────────────────

    #[test]
    fn test_default_base_url() {
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        assert_eq!(OllamaProvider::base_url(&config), "http://localhost:11434");
    }

    #[test]
    fn test_custom_base_url() {
        let config = ProviderConfig {
            api_key: None,
            base_url: Some("http://myhost:12345".to_string()),
            model: None,
        };
        assert_eq!(OllamaProvider::base_url(&config), "http://myhost:12345");
    }

    #[test]
    fn test_empty_base_url_falls_back_to_default() {
        let config = ProviderConfig {
            api_key: None,
            base_url: Some(String::new()),
            model: None,
        };
        assert_eq!(OllamaProvider::base_url(&config), "http://localhost:11434");
    }

    // ── Model resolution ───────────────────────────────────────────────

    #[test]
    fn test_model_from_step_overrides_config() {
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: Some("config-model".to_string()),
        };
        let step = ResolvedStep {
            name: "test".into(),
            description: "test".into(),
            model: Some("step-model".to_string()),
            allowed_tools: None,
            allowed_tools_list: vec![],
            budget: None,
            env: HashMap::new(),
            system_prompt: None,
            mcp_servers: None,
            agents: None,
            agent: None,
            settings: None,
        };
        assert_eq!(OllamaProvider::model(&config, &step), "step-model");
    }

    #[test]
    fn test_model_from_config_when_step_has_none() {
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: Some("config-model".to_string()),
        };
        assert_eq!(OllamaProvider::model(&config, &test_step()), "config-model");
    }

    #[test]
    fn test_model_default_when_both_none() {
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        assert_eq!(OllamaProvider::model(&config, &test_step()), DEFAULT_MODEL);
    }

    // ── Empty response ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_empty_response_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("")
                    .insert_header("content-type", "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let provider = OllamaProvider;
        let config = test_config(&server.uri());
        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.content, "");
        assert_eq!(output.exit_code, 0);
    }

    // ── Multi-chunk streaming ──────────────────────────────────────────

    #[tokio::test]
    async fn test_many_chunks_accumulated() {
        let server = MockServer::start().await;

        let mut chunks: Vec<(&str, bool)> = (0..25).map(|_| ("x", false)).collect();
        chunks.push(("!", true));
        let body = ndjson_body(&chunks);

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .insert_header("content-type", "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let provider = OllamaProvider;
        let config = test_config(&server.uri());
        let result = provider
            .execute(&test_step(), &test_task_context(), &config)
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        // 25 "x" + 1 "!"
        assert_eq!(output.content, "xxxxxxxxxxxxxxxxxxxxxxxxx!");
    }
}
