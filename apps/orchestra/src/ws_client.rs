use crate::api::ProjectsApi;
use crate::chat;
use crate::git::WorktreeManager;
use crate::ws_protocol::WsMessage;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub type WsSender = mpsc::UnboundedSender<WsMessage>;

/// Run the WebSocket client loop with automatic reconnection.
///
/// Connects to `{api_url}/agents/{agent_id}/ws`, dispatches incoming
/// chat and git requests, and sends responses back up the same connection.
/// On disconnect, waits 5 seconds and reconnects. Exits when `shutdown` is set.
pub async fn run_ws_loop(
    api_url: &str,
    agent_id: &str,
    api: ProjectsApi,
    projects_path: PathBuf,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match connect_and_run(api_url, agent_id, &api, &projects_path, &shutdown).await {
            Ok(()) => {}
            Err(e) => {
                warn!("WebSocket disconnected: {e:#}");
            }
        }
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        info!("reconnecting WebSocket in 5s...");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn connect_and_run(
    api_url: &str,
    agent_id: &str,
    api: &ProjectsApi,
    projects_path: &Path,
    shutdown: &AtomicBool,
) -> Result<()> {
    // Build WebSocket URL: http(s) -> ws(s)
    let ws_url = api_url
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    let ws_url = format!("{ws_url}/agents/{agent_id}/ws");

    info!("connecting WebSocket to {ws_url}");

    // Auth headers — same env vars as ProjectsApi
    let api_token = std::env::var("DIRAIGENT_API_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    let dev_user_id = std::env::var("DIRAIGENT_DEV_USER_ID")
        .ok()
        .filter(|s| !s.is_empty());

    let host = extract_host(&ws_url);
    let mut http_request = http::Request::builder()
        .uri(&ws_url)
        .header("Host", &host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        );

    if let Some(ref uid) = dev_user_id {
        http_request = http_request.header("X-Dev-User-Id", uid);
    } else if let Some(ref token) = api_token {
        http_request = http_request.header("Authorization", format!("Bearer {token}"));
    }

    let request = http_request.body(()).context("build WS request")?;

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("WebSocket connection failed")?;

    info!("WebSocket connected");

    let (ws_sink, mut ws_source) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

    // Internal channel for WS ping frames (separate from app-level messages)
    let (ping_tx, mut ping_rx) = mpsc::unbounded_channel::<()>();

    // Writer task: channel -> WS (handles both app messages and ping frames)
    let write_task = tokio::spawn(async move {
        let mut ws_sink = ws_sink;
        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    let text = match serde_json::to_string(&msg) {
                        Ok(t) => t,
                        Err(e) => {
                            warn!(error = %e, "failed to serialize WS message");
                            continue;
                        }
                    };
                    use tokio_tungstenite::tungstenite::Message;
                    if ws_sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Some(()) = ping_rx.recv() => {
                    use tokio_tungstenite::tungstenite::Message;
                    if ws_sink.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Heartbeat task: sends both app-level heartbeat (30s) and WS ping frames (20s)
    let hb_tx = tx.clone();
    let hb_shutdown = Arc::new(AtomicBool::new(false));
    let hb_sd = hb_shutdown.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut hb_interval = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(20));
        loop {
            tokio::select! {
                _ = hb_interval.tick() => {
                    if hb_sd.load(Ordering::SeqCst) {
                        break;
                    }
                    if hb_tx.send(WsMessage::Heartbeat).is_err() {
                        break;
                    }
                }
                _ = ping_interval.tick() => {
                    if hb_sd.load(Ordering::SeqCst) {
                        break;
                    }
                    if ping_tx.send(()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Reader loop
    while let Some(msg) = ws_source.next().await {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("WS read error: {e}");
                break;
            }
        };

        use tokio_tungstenite::tungstenite::Message as TMsg;
        match msg {
            TMsg::Text(text) => {
                let ws_msg: WsMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = %e, "malformed WS message");
                        continue;
                    }
                };

                match ws_msg {
                    WsMessage::ChatRequest {
                        session_id,
                        project_id,
                        user_id: _user_id,
                        messages,
                        system_prompt,
                        model,
                    } => {
                        let sender = tx.clone();
                        let api_clone = api.clone();
                        let pp = projects_path.to_path_buf();
                        tokio::spawn(async move {
                            // Convert ws_protocol::Message -> chat::Message
                            let chat_messages: Vec<chat::Message> = messages
                                .into_iter()
                                .map(|m| chat::Message {
                                    role: m.role,
                                    content: m.content,
                                })
                                .collect();

                            chat::handle_chat_request_ws(
                                sender,
                                &session_id,
                                &project_id.to_string(),
                                chat_messages,
                                &system_prompt,
                                &model,
                                &api_clone,
                                &pp,
                            )
                            .await;
                        });
                    }
                    WsMessage::PlanRequest {
                        request_id,
                        project_id: _project_id,
                        title,
                        description,
                        success_criteria,
                        project_name,
                    } => {
                        let sender = tx.clone();
                        tokio::spawn(async move {
                            handle_plan_request(
                                sender,
                                &request_id,
                                &title,
                                &description,
                                &success_criteria,
                                &project_name,
                            )
                            .await;
                        });
                    }
                    WsMessage::GitRequest {
                        request_id,
                        project_id,
                        query_type,
                        prefix,
                        task_id,
                        branch,
                        remote,
                        path,
                        git_ref,
                    } => {
                        let sender = tx.clone();
                        let pp = projects_path.to_path_buf();
                        let api_clone = api.clone();
                        tokio::task::spawn_blocking(move || {
                            let rt = tokio::runtime::Handle::current();

                            // Resolve project paths (git_root + working_dir)
                            let paths = rt.block_on(async {
                                crate::project_paths::resolve_project_paths(
                                    &api_clone,
                                    &project_id.to_string(),
                                    &pp,
                                )
                                .await
                            });
                            let (git_mode, git_root, working_dir, auto_push, default_branch) =
                                match paths {
                                    Ok(p) => (
                                        p.git_mode,
                                        p.git_root,
                                        p.working_dir,
                                        p.auto_push,
                                        p.default_branch,
                                    ),
                                    Err(e) => {
                                        warn!(
                                            project_id = %project_id,
                                            error = %e,
                                            "failed to resolve project paths, falling back to projects_path"
                                        );
                                        (
                                            "standalone".to_string(),
                                            Some(pp.clone()),
                                            pp.clone(),
                                            false,
                                            "main".to_string(),
                                        )
                                    }
                                };

                            // For git_mode=none, skip all git operations
                            let wm = if git_mode == "none" {
                                WorktreeManager::disabled(&working_dir)
                            } else {
                                // Use git_root for provisioning and WorktreeManager;
                                // working_dir may be a monorepo subdirectory.
                                let root = git_root.as_deref().unwrap_or(&working_dir);

                                // Auto-provision repo if it doesn't exist yet
                                if !root.join(".git").exists() {
                                    info!(
                                        project_id = %project_id,
                                        git_root = %root.display(),
                                        "git request: repo not found, provisioning..."
                                    );
                                    // Fetch project record for repo_url/slug
                                    if let Ok(project) =
                                        rt.block_on(api_clone.get_project(&project_id.to_string()))
                                    {
                                        let repo_url = project["repo_url"].as_str().unwrap_or("");
                                        let slug = project["slug"].as_str().unwrap_or("");
                                        crate::git_provisioner::provision_repo(
                                            root,
                                            repo_url,
                                            &default_branch,
                                            slug,
                                        );
                                    }
                                }

                                let m = WorktreeManager::with_branch(root, &default_branch);
                                m.set_auto_push(auto_push);
                                m
                            };
                            let response = crate::git_handler::handle_git_request_with_events(
                                &wm,
                                &api_clone,
                                &project_id.to_string(),
                                &query_type,
                                prefix.as_deref(),
                                task_id.as_deref(),
                                branch.as_deref(),
                                remote.as_deref(),
                                path.as_deref(),
                                git_ref.as_deref(),
                            );

                            let ws_response = WsMessage::GitResponse {
                                request_id,
                                success: response.success,
                                error: response.error,
                                data: response.data,
                            };

                            if let Err(e) = sender.send(ws_response) {
                                error!("failed to send git response via WS: {e}");
                            }
                        });
                    }
                    _ => {
                        warn!("unexpected WS message type from API");
                    }
                }
            }
            TMsg::Close(_) => break,
            TMsg::Ping(_) => {
                // tungstenite auto-responds to pings
            }
            _ => {}
        }
    }

    hb_shutdown.store(true, Ordering::SeqCst);
    heartbeat_task.abort();
    write_task.abort();

    Ok(())
}

fn extract_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Handle a plan.request by spawning `claude -p` with the planning prompt.
async fn handle_plan_request(
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

    // Build planning prompt (same logic as the old ai.rs)
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

Respond with ONLY a valid JSON object in this exact format, no markdown fences or extra text:
{{"tasks": [{{"title": "...", "kind": "...", "spec": "...", "acceptance_criteria": ["..."], "depends_on": []}}, ...]}}"#
    );

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
            "1",
            "--max-tokens",
            "16000",
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
                } else if accumulated_text.is_empty() {
                    accumulated_text = event["result"].as_str().unwrap_or("").to_string();
                }
                break;
            }
            _ => {}
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
        let mut msg = "Claude CLI returned empty response".to_string();
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
            msg.push_str(&format!(": {truncated}"));
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
