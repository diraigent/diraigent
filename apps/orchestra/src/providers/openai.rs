//! OpenAI provider — calls the OpenAI-compatible chat completions API.
//!
//! Sends requests to `{base_url}/v1/chat/completions` with streaming enabled,
//! parses SSE chunks, and accumulates the response content into [`StepOutput`].
//!
//! Error mapping:
//! - HTTP 401 → auth error (exit_code 1)
//! - HTTP 429 → rate-limit error (exit_code 2)
//! - HTTP 404 → model-not-found error (exit_code 3)

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

const DEFAULT_BASE_URL: &str = "https://api.openai.com";
const DEFAULT_MODEL: &str = "gpt-4o";

/// Provider that executes steps via OpenAI-compatible APIs.
pub struct OpenAIProvider;

// ── Request types ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

// ── Response types (streaming) ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: Delta,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

// ── Implementation ────────────────────────────────────────────────────────

#[async_trait]
impl StepProvider for OpenAIProvider {
    async fn execute(
        &self,
        step: &ResolvedStep,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<StepOutput> {
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/');
        let model = step
            .model
            .as_deref()
            .or(config.model.as_deref())
            .unwrap_or(DEFAULT_MODEL);

        let api_key = config
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("OpenAI API key not configured"))?;

        let url = format!("{base_url}/v1/chat/completions");

        // Build the user message from the task context.
        let user_content = if let Some(ref prompt) = task.user_prompt {
            prompt.clone()
        } else {
            serde_json::json!({
                "task_id": task.task_id,
                "project_id": task.project_id,
                "project_context": task.project_context,
                "previous_step_output": task.previous_step_output,
            })
            .to_string()
        };

        let body = ChatCompletionRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: step.description.clone(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: user_content,
                },
            ],
            stream: true,
        };

        tracing::info!(
            provider = "openai",
            model = model,
            url = %url,
            task_id = %task.task_id,
            "Sending chat completion request"
        );

        let client = reqwest::Client::new();
        let mut response = client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        // Map error HTTP status codes to typed outcomes.
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => {
                    tracing::warn!(provider = "openai", "Authentication error (401)");
                    Ok(StepOutput {
                        content: format!("Authentication error: {error_body}"),
                        exit_code: 1,
                        artifacts: HashMap::from([("error_type".into(), "auth_error".into())]),
                        cost_usd: 0.0,
                        input_tokens: 0,
                        output_tokens: 0,
                        num_turns: 0,
                        stop_reason: "error".into(),
                        is_error: true,
                    })
                }
                429 => {
                    tracing::warn!(provider = "openai", "Rate limit exceeded (429)");
                    Ok(StepOutput {
                        content: format!("Rate limit exceeded: {error_body}"),
                        exit_code: 2,
                        artifacts: HashMap::from([("error_type".into(), "rate_limit".into())]),
                        cost_usd: 0.0,
                        input_tokens: 0,
                        output_tokens: 0,
                        num_turns: 0,
                        stop_reason: "error".into(),
                        is_error: true,
                    })
                }
                404 => {
                    tracing::warn!(provider = "openai", model = model, "Model not found (404)");
                    Ok(StepOutput {
                        content: format!("Model not found: {model} — {error_body}"),
                        exit_code: 3,
                        artifacts: HashMap::from([("error_type".into(), "model_not_found".into())]),
                        cost_usd: 0.0,
                        input_tokens: 0,
                        output_tokens: 0,
                        num_turns: 0,
                        stop_reason: "error".into(),
                        is_error: true,
                    })
                }
                code => {
                    tracing::error!(
                        provider = "openai",
                        status = code,
                        "Unexpected error from OpenAI API"
                    );
                    Ok(StepOutput {
                        content: format!("Unexpected HTTP {code}: {error_body}"),
                        exit_code: 4,
                        artifacts: HashMap::from([(
                            "error_type".into(),
                            "unexpected_error".into(),
                        )]),
                        cost_usd: 0.0,
                        input_tokens: 0,
                        output_tokens: 0,
                        num_turns: 0,
                        stop_reason: "error".into(),
                        is_error: true,
                    })
                }
            };
        }

        // ── Stream SSE chunks ─────────────────────────────────────────────
        let mut content = String::new();
        let mut line_buf = String::new();

        while let Some(chunk) = response.chunk().await? {
            let text = String::from_utf8_lossy(&chunk);
            line_buf.push_str(&text);

            // Process complete lines.
            while let Some(newline_pos) = line_buf.find('\n') {
                let line = line_buf[..newline_pos].trim_end_matches('\r').to_string();
                line_buf = line_buf[newline_pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        tracing::debug!(provider = "openai", "Received [DONE] sentinel");
                        continue;
                    }
                    match serde_json::from_str::<ChatCompletionChunk>(data) {
                        Ok(parsed) => {
                            for choice in &parsed.choices {
                                if let Some(ref delta_text) = choice.delta.content {
                                    content.push_str(delta_text);
                                    tracing::trace!(
                                        provider = "openai",
                                        content_len = content.len(),
                                        "Incremental content update"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::trace!(
                                provider = "openai",
                                error = %e,
                                data = data,
                                "Failed to parse SSE chunk (non-fatal)"
                            );
                        }
                    }
                }
            }
        }

        // Process any remaining data in the buffer (no trailing newline).
        if !line_buf.is_empty() {
            let line = line_buf.trim_end_matches('\r');
            if let Some(data) = line.strip_prefix("data: ")
                && data != "[DONE]"
                && let Ok(parsed) = serde_json::from_str::<ChatCompletionChunk>(data)
            {
                for choice in &parsed.choices {
                    if let Some(ref delta_text) = choice.delta.content {
                        content.push_str(delta_text);
                    }
                }
            }
        }

        tracing::info!(
            provider = "openai",
            content_len = content.len(),
            task_id = %task.task_id,
            "Chat completion finished"
        );

        Ok(StepOutput {
            content,
            exit_code: 0,
            artifacts: Default::default(),
            cost_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            num_turns: 0,
            stop_reason: "end_turn".into(),
            is_error: false,
        })
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_step() -> ResolvedStep {
        ResolvedStep {
            name: "test".into(),
            description: "You are a test assistant.".into(),
            model: Some("gpt-4o-test".into()),
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
            task_id: "task-123".into(),
            project_id: "proj-456".into(),
            project_context: r#"{"spec":"do stuff"}"#.into(),
            previous_step_output: None,
            working_dir: None,
            log_file: None,
            user_prompt: None,
        }
    }

    /// Build an SSE body with the given text chunks.
    fn sse_body(chunks: &[&str]) -> String {
        let mut body = String::new();
        for (i, text) in chunks.iter().enumerate() {
            let chunk = serde_json::json!({
                "id": format!("chatcmpl-{i}"),
                "object": "chat.completion.chunk",
                "choices": [{
                    "index": 0,
                    "delta": { "content": text },
                    "finish_reason": serde_json::Value::Null
                }]
            });
            body.push_str(&format!("data: {chunk}\n\n"));
        }
        // Final chunk with finish_reason
        let done_chunk = serde_json::json!({
            "id": "chatcmpl-done",
            "object": "chat.completion.chunk",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        body.push_str(&format!("data: {done_chunk}\n\n"));
        body.push_str("data: [DONE]\n\n");
        body
    }

    #[tokio::test]
    async fn streaming_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer sk-test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body(&["Hello", ", ", "world", "!"]))
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = OpenAIProvider;
        let config = ProviderConfig {
            api_key: Some("sk-test-key".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok(), "should succeed: {result:?}");

        let output = result.unwrap();
        assert_eq!(output.content, "Hello, world!");
        assert_eq!(output.exit_code, 0);
        assert!(output.artifacts.is_empty());
    }

    #[tokio::test]
    async fn auth_error_401() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string(
                r#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#,
            ))
            .mount(&server)
            .await;

        let provider = OpenAIProvider;
        let config = ProviderConfig {
            api_key: Some("sk-bad-key".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.exit_code, 1);
        assert!(output.content.contains("Authentication error"));
        assert_eq!(
            output.artifacts.get("error_type").map(String::as_str),
            Some("auth_error")
        );
    }

    #[tokio::test]
    async fn rate_limit_429() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_string(
                r#"{"error":{"message":"Rate limit reached","type":"rate_limit_error"}}"#,
            ))
            .mount(&server)
            .await;

        let provider = OpenAIProvider;
        let config = ProviderConfig {
            api_key: Some("sk-test-key".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(
            output.exit_code != 0,
            "exit_code should be non-zero for rate limit"
        );
        assert_eq!(output.exit_code, 2);
        assert!(output.content.contains("Rate limit"));
        assert_eq!(
            output.artifacts.get("error_type").map(String::as_str),
            Some("rate_limit")
        );
    }

    #[tokio::test]
    async fn model_not_found_404() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(404).set_body_string(
                r#"{"error":{"message":"The model does not exist","type":"invalid_request_error"}}"#,
            ))
            .mount(&server)
            .await;

        let provider = OpenAIProvider;
        let config = ProviderConfig {
            api_key: Some("sk-test-key".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.exit_code != 0);
        assert!(output.content.contains("Model not found"));
        assert_eq!(
            output.artifacts.get("error_type").map(String::as_str),
            Some("model_not_found")
        );
    }

    #[tokio::test]
    async fn missing_api_key_returns_error() {
        let provider = OpenAIProvider;
        let config = ProviderConfig {
            api_key: None,
            base_url: Some("http://localhost:1234".into()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err(), "missing API key should return Err");

        let err = result.err().unwrap();
        assert!(
            err.to_string().contains("API key"),
            "error should mention API key: {err}"
        );
    }

    #[tokio::test]
    async fn default_base_url_is_openai() {
        assert_eq!(DEFAULT_BASE_URL, "https://api.openai.com");
    }

    #[tokio::test]
    async fn uses_config_model_when_step_model_is_none() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body(&["ok"]))
                    .insert_header("content-type", "text/event-stream"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let provider = OpenAIProvider;
        let mut step = test_step();
        step.model = None; // no step-level model
        let config = ProviderConfig {
            api_key: Some("sk-test-key".into()),
            base_url: Some(server.uri()),
            model: Some("gpt-3.5-turbo".into()), // config-level model
        };

        let result = provider.execute(&step, &test_task(), &config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "ok");
    }

    #[tokio::test]
    async fn empty_response_body() {
        let server = MockServer::start().await;

        // Server returns 200 with just the DONE sentinel and no content chunks
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("data: [DONE]\n\n")
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = OpenAIProvider;
        let config = ProviderConfig {
            api_key: Some("sk-test-key".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.content, "");
        assert_eq!(output.exit_code, 0);
    }
}
