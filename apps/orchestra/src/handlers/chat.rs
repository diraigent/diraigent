use crate::project::api::ProjectsApi;
use crate::providers::{ProviderConfig, ProviderFactory, ResolvedStep, TaskContext};
use crate::ws::protocol::{ChatSseEvent, DoneMessage, WsMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

/// Default max tokens for message history before compression kicks in.
const DEFAULT_MAX_MESSAGE_TOKENS: usize = 80_000;
/// Fraction of budget reserved for recent messages (the rest is for the summary).
const RECENT_BUDGET_FRACTION: f64 = 0.80;
/// TTL for cached project metadata (seconds).
const PROJECT_CACHE_TTL_SECS: u64 = 120;

// ── Per-project metadata cache ──────────────────────────────────────────────

struct CachedProjectInfo {
    chat_provider: String,
    chat_model: Option<String>,
    working_dir: PathBuf,
    fetched_at: Instant,
}

static PROJECT_CACHE: LazyLock<RwLock<HashMap<String, CachedProjectInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Resolve chat provider, model, and working directory for a project.
/// Results are cached for 2 minutes to avoid redundant API calls per chat message.
async fn resolve_project_info(
    api: &ProjectsApi,
    project_id: &str,
    projects_path: &Path,
) -> (String, Option<String>, PathBuf) {
    // Check cache first
    {
        let cache = PROJECT_CACHE.read().await;
        if let Some(entry) = cache
            .get(project_id)
            .filter(|e| e.fetched_at.elapsed().as_secs() < PROJECT_CACHE_TTL_SECS)
        {
            return (
                entry.chat_provider.clone(),
                entry.chat_model.clone(),
                entry.working_dir.clone(),
            );
        }
    }

    // Cache miss or expired — fetch fresh data
    let (chat_provider, chat_model) = match api.get_project(project_id).await {
        Ok(project) => {
            let provider = project["metadata"]["chat_provider"]
                .as_str()
                .unwrap_or("claude-code")
                .to_string();
            let model = project["metadata"]["chat_model"]
                .as_str()
                .map(|s| s.to_string());
            (provider, model)
        }
        Err(e) => {
            warn!("chat: failed to fetch project metadata: {e}, using default provider");
            ("claude-code".to_string(), None)
        }
    };

    let working_dir = crate::project::paths::resolve_working_dir(api, project_id, projects_path)
        .await
        .unwrap_or_else(|e| {
            warn!(
                project_id = %project_id,
                error = %e,
                "failed to resolve project working dir, falling back to projects_path"
            );
            projects_path.to_path_buf()
        });

    // Store in cache
    {
        let mut cache = PROJECT_CACHE.write().await;
        cache.insert(
            project_id.to_string(),
            CachedProjectInfo {
                chat_provider: chat_provider.clone(),
                chat_model: chat_model.clone(),
                working_dir: working_dir.clone(),
                fetched_at: Instant::now(),
            },
        );
    }

    (chat_provider, chat_model, working_dir)
}

// ── Types matching the API's chat types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

type WsSender = mpsc::UnboundedSender<WsMessage>;

// ── Handle a single chat request (called from WS client) ──

#[allow(clippy::too_many_arguments)]
pub async fn handle_chat_request_ws(
    sender: WsSender,
    session_id: &str,
    project_id: &str,
    messages: Vec<Message>,
    system_prompt: &str,
    model: &str,
    api: &ProjectsApi,
    projects_path: &Path,
) {
    let session_id = session_id.to_string();

    let send_event = |sender: WsSender, session_id: String, event: ChatSseEvent| async move {
        let ws_msg = WsMessage::ChatEvent { session_id, event };
        if let Err(e) = sender.send(ws_msg) {
            error!("failed to send chat event via WS: {e}");
        }
    };

    // Resolve chat provider, model, and working directory (cached for 2 min)
    let (chat_provider, metadata_model, working_dir) =
        resolve_project_info(api, project_id, projects_path).await;

    // Model priority: client override (from WS message) > project metadata > original model param
    let resolved_model = if model.is_empty() {
        metadata_model.unwrap_or_else(|| model.to_string())
    } else {
        model.to_string()
    };

    // Compress messages if they exceed the token budget
    let messages = compress_messages(messages, api, project_id).await;

    // Build user prompt from (possibly compressed) conversation history
    let user_prompt = build_user_prompt(&messages);

    // If chat_provider is not "claude-code", use the provider abstraction
    if chat_provider != "claude-code" {
        handle_chat_via_provider(
            sender,
            &session_id,
            project_id,
            &user_prompt,
            system_prompt,
            &resolved_model,
            &chat_provider,
            api,
        )
        .await;
        return;
    }

    // Spawn Claude Code CLI
    let mut child = match Command::new("claude")
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--no-session-persistence",
            "--model",
            &resolved_model,
            "--system-prompt",
            system_prompt,
            "--tools",
            "Bash,Read,WebFetch,WebSearch",
            "--permission-mode",
            "bypassPermissions",
        ])
        .current_dir(&working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            send_event(
                sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("Failed to spawn claude CLI: {e}"),
                },
            )
            .await;
            return;
        }
    };

    // Write user prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(user_prompt.as_bytes()).await {
            send_event(
                sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("Failed to write to claude stdin: {e}"),
                },
            )
            .await;
            return;
        }
        drop(stdin); // Close stdin to signal EOF
    }

    // Read streaming JSON from stdout
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            send_event(
                sender,
                session_id,
                ChatSseEvent::Error {
                    message: "Failed to capture claude stdout".into(),
                },
            )
            .await;
            return;
        }
    };

    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut accumulated_text = String::new();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let event: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let event_type = event["type"].as_str().unwrap_or("");

        match event_type {
            "stream_event" => {
                let inner = &event["event"];
                let inner_type = inner["type"].as_str().unwrap_or("");

                match inner_type {
                    "content_block_delta" => {
                        let delta_type = inner["delta"]["type"].as_str().unwrap_or("");
                        if delta_type == "text_delta"
                            && let Some(text) = inner["delta"]["text"].as_str()
                        {
                            accumulated_text.push_str(text);
                            send_event(
                                sender.clone(),
                                session_id.clone(),
                                ChatSseEvent::Text {
                                    content: text.to_string(),
                                },
                            )
                            .await;
                        } else if delta_type == "thinking_delta"
                            && let Some(text) = inner["delta"]["thinking"].as_str()
                        {
                            send_event(
                                sender.clone(),
                                session_id.clone(),
                                ChatSseEvent::Thinking {
                                    content: text.to_string(),
                                },
                            )
                            .await;
                        }
                    }
                    "content_block_start" => {
                        let block_type = inner["content_block"]["type"].as_str().unwrap_or("");
                        if block_type == "tool_use" {
                            let tool_name = inner["content_block"]["name"]
                                .as_str()
                                .unwrap_or("unknown")
                                .to_string();
                            let tool_id = inner["content_block"]["id"]
                                .as_str()
                                .unwrap_or("")
                                .to_string();
                            send_event(
                                sender.clone(),
                                session_id.clone(),
                                ChatSseEvent::ToolStart { tool_name, tool_id },
                            )
                            .await;
                        }
                    }
                    _ => {}
                }
            }

            "tool_use" => {
                let tool_name = event["tool"].as_str().unwrap_or("unknown").to_string();
                let tool_id = event["uuid"].as_str().unwrap_or("").to_string();
                send_event(
                    sender.clone(),
                    session_id.clone(),
                    ChatSseEvent::ToolStart { tool_name, tool_id },
                )
                .await;
            }

            "tool_result" => {
                let tool_id = event["uuid"].as_str().unwrap_or("").to_string();
                let is_error = event["is_error"].as_bool().unwrap_or(false);
                send_event(
                    sender.clone(),
                    session_id.clone(),
                    ChatSseEvent::ToolEnd {
                        tool_id,
                        success: !is_error,
                    },
                )
                .await;
            }

            "assistant" => {
                // Only set accumulated_text from the assistant event if we haven't
                // already accumulated text from streaming deltas. The assistant event
                // joins text blocks with "\n" which can differ from the delta-accumulated
                // text, causing the frontend to treat them as different messages.
                if accumulated_text.is_empty()
                    && let Some(content) = event["message"]["content"].as_array()
                {
                    let full_text: String = content
                        .iter()
                        .filter_map(|block| {
                            if block["type"].as_str() == Some("text") {
                                block["text"].as_str().map(String::from)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !full_text.is_empty() {
                        accumulated_text = full_text;
                    }
                }
            }

            "result" => {
                let is_error = event["is_error"].as_bool().unwrap_or(false);
                if is_error {
                    let error_msg = event["result"]
                        .as_str()
                        .unwrap_or("Unknown error")
                        .to_string();
                    send_event(
                        sender.clone(),
                        session_id.clone(),
                        ChatSseEvent::Error { message: error_msg },
                    )
                    .await;
                } else {
                    let final_text = if accumulated_text.is_empty() {
                        event["result"].as_str().unwrap_or("").to_string()
                    } else {
                        accumulated_text.clone()
                    };

                    send_event(
                        sender.clone(),
                        session_id.clone(),
                        ChatSseEvent::Done {
                            message: DoneMessage {
                                role: "assistant".into(),
                                content: final_text,
                            },
                        },
                    )
                    .await;
                }
                break;
            }

            _ => {}
        }
    }

    // Wait for process to finish
    let _ = child.wait().await;
    info!("chat session {session_id} completed");
}

fn build_user_prompt(messages: &[Message]) -> String {
    if messages.len() == 1 {
        return messages[0].content.clone();
    }

    let mut prompt = String::from("Conversation so far:\n\n");
    for (i, msg) in messages.iter().enumerate() {
        let role = if msg.role == "user" {
            "User"
        } else {
            "Assistant"
        };
        prompt.push_str(&format!("{role}: {}\n\n", msg.content));
        if i < messages.len() - 1 {
            prompt.push_str("---\n\n");
        }
    }
    prompt
}

// ── Context compression ──

/// Rough token estimate: ~4 characters per token for English text.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

fn max_message_tokens() -> usize {
    std::env::var("CHAT_MAX_MESSAGE_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_MESSAGE_TOKENS)
}

/// Compress messages to fit within the token budget.
///
/// Returns the original messages if they fit. Otherwise keeps the most recent
/// messages that fit within the budget and either summarizes or truncates the
/// older ones depending on whether the Claude CLI is available.
async fn compress_messages(
    messages: Vec<Message>,
    api: &ProjectsApi,
    project_id: &str,
) -> Vec<Message> {
    let budget = max_message_tokens();
    let total_tokens: usize = messages.iter().map(|m| estimate_tokens(&m.content)).sum();

    if total_tokens <= budget {
        return messages;
    }

    info!(
        total_tokens,
        budget, "chat history exceeds token budget -- compressing"
    );

    let recent_budget = (budget as f64 * RECENT_BUDGET_FRACTION) as usize;

    // Walk backwards to find how many recent messages fit.
    let mut recent_tokens = 0;
    let mut split_index = messages.len();
    for (i, msg) in messages.iter().enumerate().rev() {
        let msg_tokens = estimate_tokens(&msg.content);
        if recent_tokens + msg_tokens > recent_budget {
            split_index = i + 1;
            break;
        }
        recent_tokens += msg_tokens;
    }

    // Always keep at least the last message.
    if split_index >= messages.len() {
        split_index = messages.len() - 1;
    }

    let (older, recent) = messages.split_at(split_index);

    if older.is_empty() {
        return recent.to_vec();
    }

    // Try summarization via provider, fall back to truncation note.
    let summary = match summarize_via_provider(older, api, project_id).await {
        Some(s) => s,
        None => build_truncation_summary(older),
    };

    let mut compressed = Vec::with_capacity(1 + recent.len());
    compressed.push(Message {
        role: "user".into(),
        content: format!(
            "[Summary of earlier conversation ({} messages omitted)]\n\n{}",
            older.len(),
            summary
        ),
    });
    compressed.extend_from_slice(recent);

    let new_tokens: usize = compressed.iter().map(|m| estimate_tokens(&m.content)).sum();
    info!(
        old_message_count = older.len() + recent.len(),
        new_message_count = compressed.len(),
        old_tokens = total_tokens,
        new_tokens,
        "compressed chat history"
    );

    compressed
}

/// Summarize older messages via the Anthropic provider (Haiku model).
///
/// Uses the provider abstraction to keep summarization model-agnostic.
async fn summarize_via_provider(
    messages: &[Message],
    api: &ProjectsApi,
    project_id: &str,
) -> Option<String> {
    let conversation_text = messages
        .iter()
        .map(|m| {
            let role = if m.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            format!("{role}: {}", m.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    // Limit the text we send for summarization to avoid huge requests.
    let max_summary_input = 60_000usize; // chars, ~15K tokens
    let truncated = if conversation_text.len() > max_summary_input {
        let mut start = conversation_text.len() - max_summary_input;
        while start < conversation_text.len() && !conversation_text.is_char_boundary(start) {
            start += 1;
        }
        &conversation_text[start..]
    } else {
        &conversation_text
    };

    let prompt = format!(
        "Summarize the following conversation concisely. \
         Preserve key topics discussed, decisions made, tasks created, \
         and any important context the user would need to continue the \
         conversation. Use bullet points.\n\n{truncated}"
    );

    let provider_name = "anthropic";
    let provider = match ProviderFactory::create(provider_name) {
        Ok(p) => p,
        Err(e) => {
            warn!("failed to create provider for summarization: {e}");
            return None;
        }
    };

    let provider_cfg = match api.resolve_provider_config(project_id, provider_name).await {
        Ok(cfg) => ProviderConfig {
            api_key: cfg["api_key"].as_str().map(String::from),
            base_url: cfg["base_url"].as_str().map(String::from),
            model: cfg["default_model"].as_str().map(String::from),
        },
        Err(e) => {
            warn!("no provider config for summarization: {e}");
            return None;
        }
    };

    let step = ResolvedStep {
        name: "summarize".into(),
        description: "You are a concise summarizer. Respond with bullet points only.".into(),
        model: Some("claude-haiku-4-5-20251001".into()),
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

    let task_ctx = TaskContext {
        task_id: "summarize".into(),
        project_id: project_id.to_string(),
        project_context: String::new(),
        previous_step_output: None,
        working_dir: None,
        log_file: None,
        user_prompt: Some(prompt),
    };

    match provider.execute(&step, &task_ctx, &provider_cfg).await {
        Ok(output) if !output.is_error && !output.content.trim().is_empty() => {
            let text = output.content.trim().to_string();
            debug!(
                summary_len = text.len(),
                input_tokens = output.input_tokens,
                output_tokens = output.output_tokens,
                "summarized older messages via provider"
            );
            Some(text)
        }
        Ok(output) if output.is_error => {
            warn!(
                error = %output.content,
                "summarization provider returned error"
            );
            None
        }
        Ok(_) => {
            warn!("summarization provider returned empty response");
            None
        }
        Err(e) => {
            warn!("summarization provider failed: {e}");
            None
        }
    }
}

/// Handle a chat request via a non-claude-code provider (anthropic, openai, ollama, etc.).
///
/// Makes streaming HTTP requests directly to each provider's API, forwarding
/// text chunks as real-time `ChatSseEvent::Text` events. This gives the user
/// immediate feedback instead of waiting for the full response.
#[allow(clippy::too_many_arguments)]
async fn handle_chat_via_provider(
    sender: mpsc::UnboundedSender<WsMessage>,
    session_id: &str,
    project_id: &str,
    user_prompt: &str,
    system_prompt: &str,
    model: &str,
    provider_name: &str,
    api: &ProjectsApi,
) {
    let session_id = session_id.to_string();

    let provider_cfg = match api.resolve_provider_config(project_id, provider_name).await {
        Ok(cfg) => ProviderConfig {
            api_key: cfg["api_key"].as_str().map(String::from),
            base_url: cfg["base_url"].as_str().map(String::from),
            model: cfg["default_model"].as_str().map(String::from),
        },
        Err(e) => {
            warn!("chat: no provider config for '{provider_name}': {e}");
            ProviderConfig {
                api_key: None,
                base_url: None,
                model: None,
            }
        }
    };

    let resolved_model = provider_cfg
        .model
        .clone()
        .unwrap_or_else(|| model.to_string());

    info!(
        provider = provider_name,
        model = %resolved_model,
        "chat: streaming via provider"
    );

    match provider_name {
        "anthropic" => {
            stream_anthropic_chat(
                sender,
                &session_id,
                user_prompt,
                system_prompt,
                &resolved_model,
                &provider_cfg,
            )
            .await;
        }
        "openai" => {
            stream_openai_chat(
                sender,
                &session_id,
                user_prompt,
                system_prompt,
                &resolved_model,
                &provider_cfg,
                "openai",
            )
            .await;
        }
        "copilot" => {
            stream_openai_chat(
                sender,
                &session_id,
                user_prompt,
                system_prompt,
                &resolved_model,
                &provider_cfg,
                "copilot",
            )
            .await;
        }
        "ollama" => {
            stream_ollama_chat(
                sender,
                &session_id,
                user_prompt,
                system_prompt,
                &resolved_model,
                &provider_cfg,
            )
            .await;
        }
        _ => {
            // Unknown provider: fall back to the non-streaming provider abstraction
            stream_fallback_provider(
                sender,
                &session_id,
                project_id,
                user_prompt,
                system_prompt,
                &resolved_model,
                provider_name,
                &provider_cfg,
            )
            .await;
        }
    }
}

// ── Streaming helper: send a chat event via WS ────────────────────────────

async fn send_chat_event(
    sender: &mpsc::UnboundedSender<WsMessage>,
    session_id: &str,
    event: ChatSseEvent,
) {
    let ws_msg = WsMessage::ChatEvent {
        session_id: session_id.to_string(),
        event,
    };
    if let Err(e) = sender.send(ws_msg) {
        error!("failed to send chat event via WS: {e}");
    }
}

// ── Streaming: Anthropic Messages API (SSE) ───────────────────────────────

/// Stream from the Anthropic Messages API with `stream: true`.
///
/// Reads SSE events, forwards `content_block_delta` text chunks in real-time,
/// and sends a final `Done` event with the accumulated content.
async fn stream_anthropic_chat(
    sender: mpsc::UnboundedSender<WsMessage>,
    session_id: &str,
    user_prompt: &str,
    system_prompt: &str,
    model: &str,
    config: &ProviderConfig,
) {
    const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
    const ANTHROPIC_VERSION: &str = "2023-06-01";
    const DEFAULT_MAX_TOKENS: u32 = 16384;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or(DEFAULT_BASE_URL)
        .trim_end_matches('/');

    let api_key = match config.api_key.as_deref() {
        Some(key) => key,
        None => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: "Anthropic API key not configured".into(),
                },
            )
            .await;
            return;
        }
    };

    let url = format!("{base_url}/v1/messages");
    let body = serde_json::json!({
        "model": model,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "system": system_prompt,
        "stream": true,
        "messages": [{"role": "user", "content": user_prompt}]
    });

    let client = reqwest::Client::new();
    let response = match client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("Anthropic request failed: {e}"),
                },
            )
            .await;
            return;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        send_chat_event(
            &sender,
            session_id,
            ChatSseEvent::Error {
                message: format!("Anthropic HTTP {status}: {error_body}"),
            },
        )
        .await;
        return;
    }

    // Parse SSE stream
    let mut accumulated = String::new();
    let mut line_buf = String::new();
    let mut response = response;

    while let Ok(Some(chunk)) = response.chunk().await {
        let text = String::from_utf8_lossy(&chunk);
        line_buf.push_str(&text);

        while let Some(newline_pos) = line_buf.find('\n') {
            let line = line_buf[..newline_pos].trim_end_matches('\r').to_string();
            line_buf = line_buf[newline_pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ")
                && let Ok(event) = serde_json::from_str::<serde_json::Value>(data)
            {
                let event_type = event["type"].as_str().unwrap_or("");
                match event_type {
                    "content_block_delta" => {
                        let delta_type = event["delta"]["type"].as_str().unwrap_or("");
                        if delta_type == "text_delta"
                            && let Some(text) = event["delta"]["text"].as_str()
                        {
                            accumulated.push_str(text);
                            send_chat_event(
                                &sender,
                                session_id,
                                ChatSseEvent::Text {
                                    content: text.to_string(),
                                },
                            )
                            .await;
                        } else if delta_type == "thinking_delta"
                            && let Some(text) = event["delta"]["thinking"].as_str()
                        {
                            send_chat_event(
                                &sender,
                                session_id,
                                ChatSseEvent::Thinking {
                                    content: text.to_string(),
                                },
                            )
                            .await;
                        }
                    }
                    "message_stop" | "message_delta" => {
                        // End of message — we'll send Done below
                    }
                    "error" => {
                        let msg = event["error"]["message"]
                            .as_str()
                            .unwrap_or("Unknown streaming error");
                        send_chat_event(
                            &sender,
                            session_id,
                            ChatSseEvent::Error {
                                message: msg.to_string(),
                            },
                        )
                        .await;
                        return;
                    }
                    _ => {}
                }
            }
        }
    }

    // Send done event
    send_chat_event(
        &sender,
        session_id,
        ChatSseEvent::Done {
            message: DoneMessage {
                role: "assistant".into(),
                content: accumulated,
            },
        },
    )
    .await;
    info!("chat session {session_id} completed via streaming anthropic");
}

// ── Streaming: OpenAI-compatible APIs (SSE) ───────────────────────────────

/// Stream from OpenAI-compatible APIs (OpenAI, Copilot) with `stream: true`.
///
/// Reads SSE chunks, forwards `delta.content` text in real-time, and sends
/// a final `Done` event with the accumulated content.
#[allow(clippy::too_many_arguments)]
async fn stream_openai_chat(
    sender: mpsc::UnboundedSender<WsMessage>,
    session_id: &str,
    user_prompt: &str,
    system_prompt: &str,
    model: &str,
    config: &ProviderConfig,
    variant: &str, // "openai" or "copilot"
) {
    let (default_base, completions_path) = match variant {
        "copilot" => ("https://models.inference.ai.azure.com", "/chat/completions"),
        _ => ("https://api.openai.com", "/v1/chat/completions"),
    };

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or(default_base)
        .trim_end_matches('/');

    let api_key = match config.api_key.as_deref() {
        Some(key) => key,
        None => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("{variant} API key not configured"),
                },
            )
            .await;
            return;
        }
    };

    let url = format!("{base_url}{completions_path}");
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]
    });

    let client = reqwest::Client::new();
    let response = match client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("{variant} request failed: {e}"),
                },
            )
            .await;
            return;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        send_chat_event(
            &sender,
            session_id,
            ChatSseEvent::Error {
                message: format!("{variant} HTTP {status}: {error_body}"),
            },
        )
        .await;
        return;
    }

    // Parse SSE stream
    let mut accumulated = String::new();
    let mut line_buf = String::new();
    let mut response = response;

    while let Ok(Some(chunk)) = response.chunk().await {
        let text = String::from_utf8_lossy(&chunk);
        line_buf.push_str(&text);

        while let Some(newline_pos) = line_buf.find('\n') {
            let line = line_buf[..newline_pos].trim_end_matches('\r').to_string();
            line_buf = line_buf[newline_pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
                    && let Some(choices) = parsed["choices"].as_array()
                {
                    for choice in choices {
                        if let Some(delta_text) = choice["delta"]["content"].as_str()
                            && !delta_text.is_empty()
                        {
                            accumulated.push_str(delta_text);
                            send_chat_event(
                                &sender,
                                session_id,
                                ChatSseEvent::Text {
                                    content: delta_text.to_string(),
                                },
                            )
                            .await;
                        }
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
            && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data)
            && let Some(choices) = parsed["choices"].as_array()
        {
            for choice in choices {
                if let Some(delta_text) = choice["delta"]["content"].as_str()
                    && !delta_text.is_empty()
                {
                    accumulated.push_str(delta_text);
                    send_chat_event(
                        &sender,
                        session_id,
                        ChatSseEvent::Text {
                            content: delta_text.to_string(),
                        },
                    )
                    .await;
                }
            }
        }
    }

    send_chat_event(
        &sender,
        session_id,
        ChatSseEvent::Done {
            message: DoneMessage {
                role: "assistant".into(),
                content: accumulated,
            },
        },
    )
    .await;
    info!("chat session {session_id} completed via streaming {variant}");
}

// ── Streaming: Ollama chat API (NDJSON) ───────────────────────────────────

/// Stream from the Ollama chat API with `stream: true`.
///
/// Reads NDJSON lines, forwards `message.content` text chunks in real-time,
/// and sends a final `Done` event with the accumulated content.
async fn stream_ollama_chat(
    sender: mpsc::UnboundedSender<WsMessage>,
    session_id: &str,
    user_prompt: &str,
    system_prompt: &str,
    model: &str,
    config: &ProviderConfig,
) {
    let base_url = config
        .base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("http://localhost:11434")
        .trim_end_matches('/');

    let url = format!("{base_url}/api/chat");
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]
    });

    let client = reqwest::Client::new();
    let response = match client.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            let msg = if e.is_connect() || e.is_timeout() {
                format!("Ollama not reachable at {base_url}: {e}")
            } else {
                format!("Ollama request failed: {e}")
            };
            send_chat_event(&sender, session_id, ChatSseEvent::Error { message: msg }).await;
            return;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();
        send_chat_event(
            &sender,
            session_id,
            ChatSseEvent::Error {
                message: format!("Ollama HTTP {status}: {error_body}"),
            },
        )
        .await;
        return;
    }

    // Read NDJSON stream chunk by chunk for real-time streaming
    let mut accumulated = String::new();
    let mut line_buf = String::new();
    let mut response = response;

    while let Ok(Some(chunk)) = response.chunk().await {
        let text = String::from_utf8_lossy(&chunk);
        line_buf.push_str(&text);

        // Process complete lines
        while let Some(newline_pos) = line_buf.find('\n') {
            let line = line_buf[..newline_pos].trim().to_string();
            line_buf = line_buf[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                // Check for error
                if let Some(err_msg) = parsed["error"].as_str() {
                    send_chat_event(
                        &sender,
                        session_id,
                        ChatSseEvent::Error {
                            message: format!("Ollama error: {err_msg}"),
                        },
                    )
                    .await;
                    return;
                }

                // Extract content from message
                if let Some(content) = parsed["message"]["content"].as_str()
                    && !content.is_empty()
                {
                    accumulated.push_str(content);
                    send_chat_event(
                        &sender,
                        session_id,
                        ChatSseEvent::Text {
                            content: content.to_string(),
                        },
                    )
                    .await;
                }

                // Check if done
                if parsed["done"].as_bool().unwrap_or(false) {
                    break;
                }
            }
        }
    }

    send_chat_event(
        &sender,
        session_id,
        ChatSseEvent::Done {
            message: DoneMessage {
                role: "assistant".into(),
                content: accumulated,
            },
        },
    )
    .await;
    info!("chat session {session_id} completed via streaming ollama");
}

// ── Fallback: non-streaming provider abstraction ──────────────────────────

/// Fallback for unknown provider types: uses the `StepProvider` trait for a
/// single-shot completion and sends the result as a single text event.
#[allow(clippy::too_many_arguments)]
async fn stream_fallback_provider(
    sender: mpsc::UnboundedSender<WsMessage>,
    session_id: &str,
    project_id: &str,
    user_prompt: &str,
    system_prompt: &str,
    model: &str,
    provider_name: &str,
    provider_cfg: &ProviderConfig,
) {
    let provider = match ProviderFactory::create(provider_name) {
        Ok(p) => p,
        Err(e) => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("Failed to create provider '{provider_name}': {e}"),
                },
            )
            .await;
            return;
        }
    };

    let step = ResolvedStep {
        name: "chat".into(),
        description: system_prompt.to_string(),
        model: Some(model.to_string()),
        allowed_tools: None,
        allowed_tools_list: vec![],
        budget: None,
        env: HashMap::new(),
        system_prompt: Some(system_prompt.to_string()),
        mcp_servers: None,
        agents: None,
        agent: None,
        settings: None,
    };

    let task_ctx = TaskContext {
        task_id: format!("chat-{session_id}"),
        project_id: project_id.to_string(),
        project_context: String::new(),
        previous_step_output: None,
        working_dir: None,
        log_file: None,
        user_prompt: Some(user_prompt.to_string()),
    };

    match provider.execute(&step, &task_ctx, provider_cfg).await {
        Ok(output) if !output.is_error => {
            let content = output.content.trim().to_string();
            if !content.is_empty() {
                send_chat_event(
                    &sender,
                    session_id,
                    ChatSseEvent::Text {
                        content: content.clone(),
                    },
                )
                .await;
            }
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Done {
                    message: DoneMessage {
                        role: "assistant".into(),
                        content,
                    },
                },
            )
            .await;
            info!("chat session {session_id} completed via fallback provider '{provider_name}'");
        }
        Ok(output) => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: output.content,
                },
            )
            .await;
        }
        Err(e) => {
            send_chat_event(
                &sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("Provider error: {e}"),
                },
            )
            .await;
        }
    }
}

/// Build a simple extractive summary when API summarization is not available.
fn build_truncation_summary(messages: &[Message]) -> String {
    let user_messages: Vec<&Message> = messages.iter().filter(|m| m.role == "user").collect();

    let mut summary = String::from("Topics discussed:\n");
    for msg in user_messages.iter().take(10) {
        // Take first ~200 chars of each user message as a topic hint.
        let preview: String = msg.content.chars().take(200).collect();
        let preview = preview.trim();
        if !preview.is_empty() {
            summary.push_str(&format!("- {preview}\n"));
        }
    }
    if user_messages.len() > 10 {
        summary.push_str(&format!(
            "- ... and {} more messages\n",
            user_messages.len() - 10
        ));
    }
    summary
}
