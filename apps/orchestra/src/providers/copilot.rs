//! GitHub Copilot provider — calls the GitHub Models inference API.
//!
//! GitHub Models exposes an OpenAI-compatible chat completions endpoint at
//! `https://models.inference.ai.azure.com/chat/completions`. This provider
//! uses the same SSE streaming protocol as the OpenAI provider.
//!
//! Auth: `GITHUB_TOKEN` as Bearer token.
//!
//! Error mapping:
//! - HTTP 401 → auth error (exit_code 1)
//! - HTTP 429 → rate-limit error (exit_code 2)
//! - HTTP 404 → model-not-found error (exit_code 3)

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

const DEFAULT_BASE_URL: &str = "https://models.inference.ai.azure.com";
const DEFAULT_MODEL: &str = "openai/gpt-4.1";

/// Provider that executes steps via the GitHub Copilot / GitHub Models inference API.
pub struct CopilotProvider;

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
impl StepProvider for CopilotProvider {
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
            .ok_or_else(|| anyhow::anyhow!("Copilot token not configured"))?;

        // Copilot / GitHub Models uses /chat/completions (no /v1/ prefix)
        let url = format!("{base_url}/chat/completions");

        let user_content = serde_json::json!({
            "task_id": task.task_id,
            "project_id": task.project_id,
            "project_context": task.project_context,
            "previous_step_output": task.previous_step_output,
        })
        .to_string();

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
            provider = "copilot",
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

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => {
                    tracing::warn!(provider = "copilot", "Authentication error (401)");
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
                    tracing::warn!(provider = "copilot", "Rate limit exceeded (429)");
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
                    tracing::warn!(provider = "copilot", model = model, "Model not found (404)");
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
                        provider = "copilot",
                        status = code,
                        "Unexpected error from Copilot API"
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

            while let Some(newline_pos) = line_buf.find('\n') {
                let line = line_buf[..newline_pos].trim_end_matches('\r').to_string();
                line_buf = line_buf[newline_pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    match serde_json::from_str::<ChatCompletionChunk>(data) {
                        Ok(parsed) => {
                            for choice in &parsed.choices {
                                if let Some(ref delta_text) = choice.delta.content {
                                    content.push_str(delta_text);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::trace!(
                                provider = "copilot",
                                error = %e,
                                "Failed to parse SSE chunk (non-fatal)"
                            );
                        }
                    }
                }
            }
        }

        // Process remaining buffer
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
            provider = "copilot",
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
            model: Some("openai/gpt-4.1-test".into()),
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
        }
    }

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
            .and(path("/chat/completions"))
            .and(header("Authorization", "Bearer ghp-test-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(sse_body(&["Hello", ", ", "world", "!"]))
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;

        let provider = CopilotProvider;
        let config = ProviderConfig {
            api_key: Some("ghp-test-token".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok(), "should succeed: {result:?}");

        let output = result.unwrap();
        assert_eq!(output.content, "Hello, world!");
        assert_eq!(output.exit_code, 0);
    }

    #[tokio::test]
    async fn auth_error_401() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_string(
                r#"{"error":{"message":"Bad credentials","type":"invalid_request_error"}}"#,
            ))
            .mount(&server)
            .await;

        let provider = CopilotProvider;
        let config = ProviderConfig {
            api_key: Some("bad-token".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 1);
        assert!(output.is_error);
    }

    #[tokio::test]
    async fn missing_token_returns_error() {
        let provider = CopilotProvider;
        let config = ProviderConfig {
            api_key: None,
            base_url: Some("http://localhost:1234".into()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("token"));
    }
}
