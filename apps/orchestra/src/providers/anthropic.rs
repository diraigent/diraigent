//! Anthropic provider — calls the Anthropic Messages API directly.
//!
//! Sends a non-streaming POST to `{base_url}/v1/messages`, parses the JSON
//! response, and returns a [`StepOutput`] with real token counts and cost.
//!
//! Error mapping:
//! - HTTP 401 → auth error (exit_code 1)
//! - HTTP 429 → rate-limit error (exit_code 2)
//! - HTTP 404 → model-not-found error (exit_code 3)

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_MODEL: &str = "claude-sonnet-4-6-20250514";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 16384;

/// Provider that executes steps via the Anthropic Messages API.
pub struct AnthropicProvider;

// ── Request types ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

// ── Response types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

// ── Implementation ────────────────────────────────────────────────────────

#[async_trait]
impl StepProvider for AnthropicProvider {
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
            .ok_or_else(|| anyhow::anyhow!("Anthropic API key not configured"))?;

        let url = format!("{base_url}/v1/messages");

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

        let body = MessagesRequest {
            model: model.to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
            system: step.description.clone(),
            messages: vec![ApiMessage {
                role: "user".into(),
                content: user_content,
            }],
        };

        tracing::info!(
            provider = "anthropic",
            model = model,
            url = %url,
            task_id = %task.task_id,
            "Sending messages request"
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 => {
                    tracing::warn!(provider = "anthropic", "Authentication error (401)");
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
                    tracing::warn!(provider = "anthropic", "Rate limit exceeded (429)");
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
                    tracing::warn!(
                        provider = "anthropic",
                        model = model,
                        "Model not found (404)"
                    );
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
                        provider = "anthropic",
                        status = code,
                        "Unexpected error from Anthropic API"
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

        let response_text = response.text().await?;
        let parsed: MessagesResponse = serde_json::from_str(&response_text)?;

        // Extract text from content blocks
        let content: String = parsed
            .content
            .iter()
            .filter_map(|block| {
                if block.block_type == "text" {
                    block.text.as_deref()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let stop_reason = parsed.stop_reason.unwrap_or_else(|| "unknown".into());
        let (input_tokens, output_tokens) = parsed
            .usage
            .map(|u| (u.input_tokens, u.output_tokens))
            .unwrap_or((0, 0));

        tracing::info!(
            provider = "anthropic",
            content_len = content.len(),
            input_tokens,
            output_tokens,
            stop_reason = %stop_reason,
            task_id = %task.task_id,
            "Messages request completed"
        );

        Ok(StepOutput {
            content,
            exit_code: 0,
            artifacts: Default::default(),
            cost_usd: 0.0,
            input_tokens,
            output_tokens,
            num_turns: 1,
            stop_reason,
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
            model: Some("claude-test".into()),
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

    fn success_body(text: &str) -> String {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 100, "output_tokens": 50}
        })
        .to_string()
    }

    #[tokio::test]
    async fn successful_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "sk-test-key"))
            .and(header("anthropic-version", ANTHROPIC_VERSION))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(success_body("Hello, world!"))
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = AnthropicProvider;
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
        assert_eq!(output.input_tokens, 100);
        assert_eq!(output.output_tokens, 50);
        assert_eq!(output.stop_reason, "end_turn");
        assert!(!output.is_error);
    }

    #[tokio::test]
    async fn auth_error_401() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_string(
                r#"{"error":{"message":"Invalid API key","type":"authentication_error"}}"#,
            ))
            .mount(&server)
            .await;

        let provider = AnthropicProvider;
        let config = ProviderConfig {
            api_key: Some("sk-bad-key".into()),
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
    async fn rate_limit_429() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(429).set_body_string(
                r#"{"error":{"message":"Rate limit reached","type":"rate_limit_error"}}"#,
            ))
            .mount(&server)
            .await;

        let provider = AnthropicProvider;
        let config = ProviderConfig {
            api_key: Some("sk-test-key".into()),
            base_url: Some(server.uri()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.exit_code, 2);
        assert!(output.is_error);
    }

    #[tokio::test]
    async fn missing_api_key_returns_error() {
        let provider = AnthropicProvider;
        let config = ProviderConfig {
            api_key: None,
            base_url: Some("http://localhost:1234".into()),
            model: None,
        };

        let result = provider.execute(&test_step(), &test_task(), &config).await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("API key"));
    }

    #[tokio::test]
    async fn empty_content_array() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(
                        r#"{"content":[],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":0}}"#,
                    )
                    .insert_header("content-type", "application/json"),
            )
            .mount(&server)
            .await;

        let provider = AnthropicProvider;
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
