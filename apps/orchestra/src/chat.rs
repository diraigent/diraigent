use crate::api::ProjectsApi;
use crate::ws_protocol::{ChatSseEvent, DoneMessage, WsMessage};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Default max tokens for message history before compression kicks in.
const DEFAULT_MAX_MESSAGE_TOKENS: usize = 80_000;
/// Fraction of budget reserved for recent messages (the rest is for the summary).
const RECENT_BUDGET_FRACTION: f64 = 0.80;

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

    // Resolve the project's working directory from the API.
    let working_dir = crate::project_paths::resolve_working_dir(api, project_id, projects_path)
        .await
        .unwrap_or_else(|e| {
            warn!(
                project_id = %project_id,
                error = %e,
                "failed to resolve project working dir, falling back to projects_path"
            );
            projects_path.to_path_buf()
        });

    // Compress messages if they exceed the token budget
    let messages = compress_messages(messages).await;

    // Build user prompt from (possibly compressed) conversation history
    let user_prompt = build_user_prompt(&messages);

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
            model,
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
async fn compress_messages(messages: Vec<Message>) -> Vec<Message> {
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

    // Try summarization via claude CLI, fall back to truncation note.
    let summary = match summarize_via_cli(older).await {
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

/// Summarize older messages via `claude -p` subprocess (Haiku model).
///
/// This routes through the normal Claude CLI infrastructure rather than making
/// direct Anthropic API calls, keeping all AI usage tracked and consistent.
async fn summarize_via_cli(messages: &[Message]) -> Option<String> {
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
        // Find a safe UTF-8 boundary to avoid panicking on multi-byte chars.
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

    let mut child = match Command::new("claude")
        .args([
            "-p",
            "--output-format",
            "text",
            "--no-session-persistence",
            "--model",
            "claude-haiku-4-5-20251001",
            "--max-turns",
            "1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            warn!("failed to spawn claude CLI for summarization: {e}");
            return None;
        }
    };

    // Write prompt and close stdin to signal EOF
    {
        let mut stdin = child.stdin.take()?;
        if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
            warn!("failed to write summarization prompt to claude stdin: {e}");
            return None;
        }
        // stdin dropped here, signaling EOF
    }

    let output = match child.wait_with_output().await {
        Ok(output) => output,
        Err(e) => {
            warn!("failed to wait for claude summarization process: {e}");
            return None;
        }
    };

    if !output.status.success() {
        warn!(
            status = ?output.status,
            "claude summarization CLI returned non-zero exit -- falling back to truncation"
        );
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if text.is_empty() {
        return None;
    }

    debug!(
        summary_len = text.len(),
        "summarized older messages via CLI"
    );
    Some(text)
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
