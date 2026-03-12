use crate::api::ProjectsApi;
use crate::chat;
use crate::git::WorktreeManager;
use crate::ws_protocol::WsMessage;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

                            // Resolve project working dir to create a WorktreeManager
                            let working_dir = rt.block_on(async {
                                crate::project_paths::resolve_working_dir(
                                    &api_clone,
                                    &project_id.to_string(),
                                    &pp,
                                )
                                .await
                                .unwrap_or_else(|e| {
                                    warn!(
                                        project_id = %project_id,
                                        error = %e,
                                        "failed to resolve git working dir, falling back to projects_path"
                                    );
                                    pp.clone()
                                })
                            });

                            // Fetch project record for default_branch and provisioning
                            let project_data = rt
                                .block_on(api_clone.get_project(&project_id.to_string()))
                                .ok();
                            let default_branch = project_data
                                .as_ref()
                                .and_then(|p| p["default_branch"].as_str())
                                .unwrap_or("main");

                            // Auto-provision repo if it doesn't exist yet
                            if !working_dir.join(".git").exists() {
                                info!(
                                    project_id = %project_id,
                                    working_dir = %working_dir.display(),
                                    "git request: repo not found, provisioning..."
                                );
                                if let Some(ref project) = project_data {
                                    let repo_url = project["repo_url"].as_str().unwrap_or("");
                                    let slug = project["slug"].as_str().unwrap_or("");
                                    crate::git_provisioner::provision_repo(
                                        &working_dir,
                                        repo_url,
                                        default_branch,
                                        slug,
                                    );
                                }
                            }

                            let auto_push = project_data
                                .as_ref()
                                .and_then(|p| p["metadata"]["auto_push"].as_bool())
                                .unwrap_or(false);

                            let wm = WorktreeManager::with_branch(&working_dir, default_branch);
                            wm.set_auto_push(auto_push);
                            let response = crate::git_handler::handle_git_request(
                                &wm,
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
