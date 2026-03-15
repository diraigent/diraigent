use crate::ws_client::WsSender;
use crate::ws_protocol::WsMessage;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Handle a plan.request by spawning `claude -p` with the planning prompt.
pub async fn handle_plan_request(
    sender: WsSender,
    request_id: &str,
    title: &str,
    description: &str,
    success_criteria: &serde_json::Value,
    project_name: &str,
) {
    let request_id = request_id.to_string();

    let send_error = |sender: WsSender, request_id: String, msg: String| async move {
        let ws_msg = WsMessage::PlanResponse {
            request_id,
            success: false,
            error: Some(msg),
            tasks: serde_json::Value::Array(vec![]),
        };
        if let Err(e) = sender.send(ws_msg) {
            error!("failed to send plan error via WS: {e}");
        }
    };

    // Build planning prompt
    let criteria_text = match success_criteria {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| format!("- {s}")))
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::String(s) => s.clone(),
        _ => success_criteria.to_string(),
    };

    let desc = if description.is_empty() {
        "No description provided"
    } else {
        description
    };
    let criteria = if criteria_text.is_empty() {
        "None specified".to_string()
    } else {
        criteria_text
    };

    let prompt = format!(
        r#"You are a technical project planner for the project "{project_name}".

Decompose the following work item into 3-8 concrete, implementable tasks. Each task should be small enough for a single developer to complete in one session.

## Work Item
**Title**: {title}
**Description**: {desc}
**Success Criteria**:
{criteria}

## Requirements
- Order tasks by dependency (tasks that must be done first come first)
- Each task must have a clear, specific scope
- kind must be one of: feature, bug, refactor, test, docs
- spec should be a concise technical description of what to implement (2-4 sentences, not a full design doc)
- acceptance_criteria should be verifiable conditions (not vague)
- depends_on is an array of zero-based indices referencing earlier tasks in this list that must complete first
- The first task must always have depends_on: [] (empty array)
- Do NOT create meta-tasks like "review" or "deploy" — only implementation work

Respond with a JSON object matching the required schema."#
    );

    let json_schema = r#"{"type":"object","properties":{"tasks":{"type":"array","items":{"type":"object","properties":{"title":{"type":"string"},"kind":{"type":"string","enum":["feature","bug","refactor","test","docs"]},"spec":{"type":"string"},"acceptance_criteria":{"type":"array","items":{"type":"string"}},"depends_on":{"type":"array","items":{"type":"integer"}}},"required":["title","kind","spec","acceptance_criteria","depends_on"]}}},"required":["tasks"]}"#;

    // Spawn claude -p
    let mut child = match Command::new("claude")
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--model",
            "sonnet",
            "--max-turns",
            "2",
            "--json-schema",
            json_schema,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            send_error(
                sender,
                request_id,
                format!("Failed to spawn claude CLI: {e}"),
            )
            .await;
            return;
        }
    };

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(prompt.as_bytes()).await {
            send_error(
                sender,
                request_id,
                format!("Failed to write to claude stdin: {e}"),
            )
            .await;
            return;
        }
        drop(stdin);
    }

    // Read streaming JSON from stdout, collect the result
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            send_error(sender, request_id, "Failed to capture claude stdout".into()).await;
            return;
        }
    };

    // Collect stderr concurrently to avoid pipe deadlocks and capture error diagnostics
    let stderr_handle = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines_iter = reader.lines();
            let mut buf = String::new();
            while let Ok(Some(line)) = lines_iter.next_line().await {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(&line);
            }
            buf
        })
    });

    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut accumulated_text = String::new();
    let mut is_error = false;
    let mut total_lines: usize = 0;
    let mut parsed_events: usize = 0;
    let mut skipped_lines: Vec<String> = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        total_lines += 1;

        let event: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                // Capture non-JSON lines for diagnostics (truncate to 200 chars)
                let preview: String = line.chars().take(200).collect();
                skipped_lines.push(preview);
                continue;
            }
        };

        parsed_events += 1;
        let event_type = event["type"].as_str().unwrap_or("");

        match event_type {
            "stream_event" => {
                let inner = &event["event"];
                if inner["type"].as_str() == Some("content_block_delta")
                    && let Some(text) = inner["delta"]["text"].as_str()
                {
                    accumulated_text.push_str(text);
                }
            }
            "assistant" => {
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
                is_error = event["is_error"].as_bool().unwrap_or(false);
                if is_error {
                    accumulated_text = event["result"]
                        .as_str()
                        .unwrap_or("Unknown error")
                        .to_string();
                } else if let Some(structured) = event.get("structured_output") {
                    // --json-schema produces structured_output in the result event
                    accumulated_text = structured.to_string();
                } else if accumulated_text.is_empty() {
                    accumulated_text = event["result"].as_str().unwrap_or("").to_string();
                }
                break;
            }
            _ => {
                debug!(event_type, "plan request: unhandled event type");
            }
        }
    }

    let exit_status = child.wait().await;

    // Collect stderr output
    let stderr_text = match stderr_handle {
        Some(handle) => handle.await.unwrap_or_default(),
        None => String::new(),
    };

    if is_error {
        send_error(sender, request_id, accumulated_text).await;
        return;
    }

    // Check for empty response before attempting JSON parse
    if accumulated_text.trim().is_empty() {
        let mut msg = format!(
            "Claude CLI returned empty response (stdout lines: {total_lines}, parsed events: {parsed_events}, skipped: {})",
            skipped_lines.len()
        );
        if let Ok(status) = &exit_status
            && !status.success()
        {
            let code = status.code().unwrap_or(-1);
            msg.push_str(&format!(" (exit code: {code})"));
        }
        let stderr_trimmed = stderr_text.trim();
        if !stderr_trimmed.is_empty() {
            // Include up to 500 chars of stderr for diagnostics
            let truncated: String = stderr_trimmed.chars().take(500).collect();
            msg.push_str(&format!(" stderr: {truncated}"));
        }
        if !skipped_lines.is_empty() {
            let skipped_preview = skipped_lines.join(" | ");
            let truncated: String = skipped_preview.chars().take(500).collect();
            msg.push_str(&format!(" non-json output: {truncated}"));
        }
        warn!(msg = %msg, "plan request: empty claude response");
        send_error(sender, request_id, msg).await;
        return;
    }

    // Parse the JSON response from Claude's text output.
    // Claude sometimes emits preamble text before the JSON or wraps it in
    // markdown fences.  We try progressively more aggressive extraction:
    //   1. Direct parse (already valid JSON)
    //   2. Strip ```json ... ``` fences
    //   3. Find the first '{' / last '}' and extract the substring
    let text = accumulated_text.trim();
    let json_text = extract_json_object(text);

    let parsed: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                error = %e,
                text = %json_text,
                "failed to parse plan response as JSON"
            );
            let preview: String = json_text.chars().take(200).collect();
            send_error(
                sender,
                request_id,
                format!("Failed to parse AI response as JSON: {e} (preview: {preview})"),
            )
            .await;
            return;
        }
    };

    // Extract the tasks array
    let tasks = if let Some(tasks) = parsed.get("tasks") {
        tasks.clone()
    } else if parsed.is_array() {
        // Claude might return just the array
        parsed
    } else {
        send_error(
            sender,
            request_id,
            "AI response did not contain a 'tasks' array".into(),
        )
        .await;
        return;
    };

    let ws_msg = WsMessage::PlanResponse {
        request_id,
        success: true,
        error: None,
        tasks,
    };

    if let Err(e) = sender.send(ws_msg) {
        error!("failed to send plan response via WS: {e}");
    }

    info!("plan request completed");
}

/// Extract a JSON object from text that may contain preamble or markdown fences.
fn extract_json_object(text: &str) -> &str {
    // 1. Already valid JSON — fast path
    if text.starts_with('{') || text.starts_with('[') {
        return text;
    }

    // 2. Strip markdown fences: ```json ... ``` or ``` ... ```
    if let Some(fence_start) = text.find("```") {
        let after_fence = &text[fence_start + 3..];
        // Skip optional language tag on the same line
        let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let inner = &after_fence[content_start..];
        if let Some(end) = inner.find("```") {
            let candidate = inner[..end].trim();
            if candidate.starts_with('{') || candidate.starts_with('[') {
                return candidate;
            }
        }
    }

    // 3. Find the outermost { ... } substring
    if let Some(start) = text.find('{')
        && let Some(end) = text.rfind('}')
        && end > start
    {
        return &text[start..=end];
    }

    // 4. Fallback — return as-is and let the caller's parse error handle it
    text
}
