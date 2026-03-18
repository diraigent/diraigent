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
        let start = conversation_text.len() - max_summary_input;
        let safe_start = conversation_text.ceil_char_boundary(start);
        &conversation_text[safe_start..]
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
/// Uses the provider abstraction for a single-shot completion, then streams
/// the result back as chat events.
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

    let send_event = |sender: mpsc::UnboundedSender<WsMessage>,
                      session_id: String,
                      event: ChatSseEvent| async move {
        let ws_msg = WsMessage::ChatEvent { session_id, event };
        if let Err(e) = sender.send(ws_msg) {
            error!("failed to send chat event via WS: {e}");
        }
    };

    let provider = match ProviderFactory::create(provider_name) {
        Ok(p) => p,
        Err(e) => {
            send_event(
                sender,
                session_id,
                ChatSseEvent::Error {
                    message: format!("Failed to create provider '{provider_name}': {e}"),
                },
            )
            .await;
            return;
        }
    };

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

    let step = ResolvedStep {
        name: "chat".into(),
        description: system_prompt.to_string(),
        model: Some(
            provider_cfg
                .model
                .clone()
                .unwrap_or_else(|| model.to_string()),
        ),
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

    info!("chat: calling provider '{provider_name}'");

    match provider.execute(&step, &task_ctx, &provider_cfg).await {
        Ok(output) if !output.is_error => {
            let content = output.content.trim().to_string();
            // Send the full text as a single text event, then done
            if !content.is_empty() {
                send_event(
                    sender.clone(),
                    session_id.clone(),
                    ChatSseEvent::Text {
                        content: content.clone(),
                    },
                )
                .await;
            }
            send_event(
                sender,
                session_id.clone(),
                ChatSseEvent::Done {
                    message: DoneMessage {
                        role: "assistant".into(),
                        content,
                    },
                },
            )
            .await;
            info!("chat session {session_id} completed via provider '{provider_name}'");
        }
        Ok(output) => {
            send_event(
                sender,
                session_id,
                ChatSseEvent::Error {
                    message: output.content,
                },
            )
            .await;
        }
        Err(e) => {
            send_event(
                sender,
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
