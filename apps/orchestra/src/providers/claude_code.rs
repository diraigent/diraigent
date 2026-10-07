//! Claude Code CLI provider — wraps the Claude Code CLI subprocess.
//!
//! Spawns `claude -p` in a PTY via `script`, reads the stream-json log file
//! for cost/token metrics, and returns a [`TaskOutput`] with full telemetry.
//!
//! Registered as both `"claude-code"` (canonical) and `"anthropic"` (legacy alias).

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use crate::engine::mcp::Sessions;
use anyhow::Context;
use async_trait::async_trait;
use http_body_util::{BodyExt, Full, Limited};
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tokio::process::Command;
use tracing::{error, warn};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::{ProviderConfig, ResolvedTask, TaskContext, TaskOutput, TaskProvider};

/// Provider that executes steps via the Claude Code CLI.
pub struct ClaudeCodeProvider;

#[async_trait]
impl TaskProvider for ClaudeCodeProvider {
    async fn execute(
        &self,
        step: &ResolvedTask,
        task: &TaskContext,
        _config: &ProviderConfig,
    ) -> anyhow::Result<TaskOutput> {
        let worktree = task
            .working_dir
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ClaudeCodeProvider requires working_dir"))?;
        let log_file = task
            .log_file
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ClaudeCodeProvider requires log_file"))?;

        let system_prompt = step.system_prompt.as_deref().unwrap_or("");
        let user_prompt = task.user_prompt.as_deref().unwrap_or(&task.project_context);

        run_claude(system_prompt, user_prompt, worktree, log_file, step).await?;
        let (cost, input_tokens, output_tokens, turns, stop, is_err, result_text) =
            parse_result_from_log(log_file).await;

        Ok(TaskOutput {
            content: result_text,
            exit_code: if is_err { 1 } else { 0 },
            artifacts: HashMap::new(),
            cost_usd: cost,
            input_tokens,
            output_tokens,
            num_turns: turns,
            stop_reason: stop,
            is_error: is_err,
        })
    }
}

async fn run_claude(
    system_prompt: &str,
    user_prompt: &str,
    worktree: &Path,
    log_file: &Path,
    config: &ResolvedTask,
) -> anyhow::Result<()> {
    // Write prompts to temp files to avoid OS ARG_MAX limits.
    // Some(empty) is still an authoritative managed registry: no ambient MCP.
    let managed = config.mcp_servers.is_some();
    if managed {
        validate_managed_options(config)?;
        // Before 2.1.246 strict headless sessions could still wait for ambient
        // project-server approval. Require the documented isolation fix.
        let version = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            Command::new("claude")
                .arg("--version")
                .envs(config.env.iter())
                .env_remove("CLAUDECODE")
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Claude version check timed out")?
        .context("check Claude version")?;
        let text = String::from_utf8_lossy(&version.stdout);
        let parsed: Option<Vec<u32>> = text.split_whitespace().next().and_then(|v| {
            v.split('.')
                .map(str::parse)
                .collect::<Result<Vec<_>, _>>()
                .ok()
        });
        if !version.status.success()
            || parsed
                .as_ref()
                .is_none_or(|v| v.len() != 3 || v.as_slice() < [2, 1, 246].as_slice())
        {
            anyhow::bail!(
                "Managed MCP requires Claude Code >= 2.1.246; upgrade for strict headless MCP isolation"
            );
        }
        let help = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            Command::new("claude")
                .arg("--help")
                .envs(config.env.iter())
                .env_remove("CLAUDECODE")
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Claude MCP capability check timed out")?
        .context("check Claude MCP capability")?;
        let text = String::from_utf8_lossy(&help.stdout);
        if !help.status.success()
            || ![
                "--strict-mcp-config",
                "--mcp-config",
                "--permission-mode",
                "--setting-sources",
                "dontAsk",
                "--no-chrome",
            ]
            .iter()
            .all(|flag| text.contains(flag))
        {
            anyhow::bail!(
                "Claude Code lacks strict MCP isolation; upgrade to a CLI supporting --strict-mcp-config, --mcp-config, --setting-sources and --permission-mode dontAsk"
            );
        }
    }
    let runtime = tempfile::Builder::new()
        .prefix("orchestra-claude-")
        .tempdir()?;
    let temp_dir = runtime.path();
    #[cfg(unix)]
    std::fs::set_permissions(temp_dir, std::fs::Permissions::from_mode(0o700))
        .context("set private runtime directory permissions")?;
    if temp_dir
        .canonicalize()?
        .starts_with(worktree.canonicalize()?)
    {
        anyhow::bail!(
            "Claude runtime configuration must be outside the worktree; configure an external OS temporary directory"
        );
    }
    let listener = if managed {
        Some(tokio::net::TcpListener::bind("127.0.0.1:0").await?)
    } else {
        None
    };
    let token = uuid::Uuid::new_v4().to_string();

    let prompt_file = temp_dir.join("prompt.txt");
    let system_file = temp_dir.join("system.txt");
    tokio::fs::write(&prompt_file, user_prompt)
        .await
        .context("write user prompt to temp file")?;
    tokio::fs::write(&system_file, system_prompt)
        .await
        .context("write system prompt to temp file")?;
    private_file(&prompt_file)?;
    private_file(&system_file)?;

    // Build --allowedTools flags
    let mut tool_rules = Vec::new();
    for tool in &config.allowed_tools_list {
        tool_rules.push(quote(tool));
    }

    let model_flag = config
        .model
        .as_deref()
        .map(|m| format!(" --model {}", quote(m)))
        .unwrap_or_default();

    let budget_flag = config
        .budget
        .map(|b| format!(" --max-budget-usd {b:.1}"))
        .unwrap_or_default();

    let mcp_flag = if let Some(listener) = &listener {
        let mut servers = serde_json::Map::new();
        for (index, connection) in config.mcp_servers.as_ref().unwrap().0.iter().enumerate() {
            let name = format!("orchestra_{index}");
            let tools = connection.tools_list().await?;
            for tool in tools["tools"]
                .as_array()
                .context("invalid broker catalog")?
            {
                let tool = tool["name"].as_str().context("invalid broker tool name")?;
                if !tool
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    || tool.is_empty()
                {
                    anyhow::bail!(
                        "Broker tool name cannot be represented as an exact Claude permission"
                    );
                }
                tool_rules.push(quote(&format!("mcp__{name}__{tool}")));
            }
            servers.insert(name, serde_json::json!({"type":"http", "url":format!("http://{}/{index}", listener.local_addr()?), "headers":{"Authorization":format!("Bearer {token}")}}));
        }
        let path = temp_dir.join("mcp.json");
        tokio::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"mcpServers": servers}))?,
        )
        .await?;
        private_file(&path)?;
        format!(
            " --strict-mcp-config --mcp-config {} --permission-mode dontAsk --setting-sources '' --no-chrome",
            quote(&path.to_string_lossy())
        )
    } else {
        String::new()
    };

    let tool_flags = if tool_rules.is_empty() {
        String::new()
    } else {
        format!(" --allowedTools {}", tool_rules.join(" "))
    };

    // Pass custom sub-agents as --agents '<json>' if specified.
    let agents_flag = if let Some(agents) = &config.agents {
        let agents_str = serde_json::to_string(agents).unwrap_or_default();
        let escaped = agents_str.replace('\'', "'\\''");
        format!(" --agents '{escaped}'")
    } else {
        String::new()
    };

    // Activate a specific named agent via --agent <name> if specified.
    let agent_flag = config
        .agent
        .as_deref()
        .map(|a| format!(" --agent {}", quote(a)))
        .unwrap_or_default();

    // Pass additional settings as --settings '<json>'.
    let settings_flag = if let Some(settings) = &config.settings {
        let settings_str = serde_json::to_string(settings).unwrap_or_default();
        let escaped = settings_str.replace('\'', "'\\''");
        format!(" --settings '{escaped}'")
    } else {
        String::new()
    };

    // Create wrapper script that pipes the user prompt via stdin.
    let wrapper_content = format!(
        "#!/bin/bash\n\
         umask 077\n\
         echo $$ > '{pidfile}'\n\
         SYSTEM=\"$(cat '{system}')\"\n\
         exec claude -p < '{prompt}' \\\n\
           --system-prompt \"$SYSTEM\" \\\n\
           --no-session-persistence \\\n\
           {permissions} \\\n\
           --output-format stream-json \\\n\
           --verbose{model}{budget}{tools}{mcp}{agents}{agent}{settings}\n",
        system = system_file.display().to_string().replace('\'', "'\\''"),
        pidfile = temp_dir
            .join("child.pid")
            .display()
            .to_string()
            .replace('\'', "'\\''"),
        prompt = prompt_file.display().to_string().replace('\'', "'\\''"),
        model = model_flag,
        budget = budget_flag,
        tools = tool_flags,
        mcp = mcp_flag,
        agents = agents_flag,
        agent = agent_flag,
        settings = settings_flag,
        permissions = if managed {
            ""
        } else {
            "--dangerously-skip-permissions"
        },
    );

    let wrapper_path = temp_dir.join("run.sh");
    tokio::fs::write(&wrapper_path, &wrapper_content)
        .await
        .context("write wrapper script")?;

    #[cfg(unix)]
    std::fs::set_permissions(&wrapper_path, std::fs::Permissions::from_mode(0o700))
        .context("set wrapper script permissions")?;

    // `script` wraps claude in a PTY for proper Node.js output flushing.
    let log_path = log_file.to_str().unwrap();
    let wrapper_str = wrapper_path.to_str().unwrap();

    let script_args = if cfg!(target_os = "macos") {
        vec![
            "-q".to_string(),
            log_path.to_string(),
            "bash".to_string(),
            wrapper_str.to_string(),
        ]
    } else {
        vec![
            "-q".to_string(),
            "-e".to_string(),
            "-c".to_string(),
            format!("bash {}", quote(wrapper_str)),
            log_path.to_string(),
        ]
    };

    let mut command = Command::new("script");
    command
        .args(&script_args)
        .current_dir(worktree)
        .env_remove("CLAUDECODE")
        .envs(config.env.iter())
        .kill_on_drop(true)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if managed {
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if (name.starts_with("CLAUDE_CODE_") && name != "CLAUDE_CODE_OAUTH_TOKEN")
                || ["NODE_OPTIONS", "BASH_ENV", "ENV"].contains(&name.as_ref())
            {
                command.env_remove(key);
            }
        }
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().context("spawn script/claude process")?;
    let _process_group = ProcessGroup(child.id(), temp_dir.join("child.pid"));

    let status = if let Some(listener) = listener {
        tokio::select! {
            status = child.wait() => status.context("wait for claude process")?,
            result = serve_broker(listener, config.mcp_servers.as_ref().unwrap(), &token) => {
                result?;
                anyhow::bail!("Claude MCP endpoint stopped unexpectedly");
            }
        }
    } else {
        child.wait().await.context("wait for claude process")?
    };

    if !status.success() {
        error!("claude exited with status {status}");
        anyhow::bail!("claude exited with status {status}");
    }
    Ok(())
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

struct ProcessGroup(Option<u32>, std::path::PathBuf);
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Ok(pid) = std::fs::read_to_string(&self.1)
            .unwrap_or_default()
            .trim()
            .parse::<i32>()
            && pid > 1
        {
            // forkpty may put its child in a separate session/group.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
                libc::kill(pid, libc::SIGKILL);
            }
        }
        #[cfg(unix)]
        if let Some(pid) = self.0 {
            // script and its wrapper must not outlive the private runtime files.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}

fn private_file(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn validate_managed_options(config: &ResolvedTask) -> anyhow::Result<()> {
    fn unsafe_tool(tool: &str) -> bool {
        let base = tool.split('(').next().unwrap_or("");
        base.is_empty()
            || !base.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || tool.to_ascii_lowercase().contains("mcp")
            || tool.contains(' ')
            || tool.contains(',')
    }
    fn unsafe_settings(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().any(|(key, value)| {
                let key = key.to_ascii_lowercase();
                key.contains("mcp")
                    || key.contains("permission")
                    || ["hooks", "env", "plugins", "enabledplugins"].contains(&key.as_str())
                    || (key == "tools"
                        && value.as_array().is_none_or(|tools| {
                            tools
                                .iter()
                                .any(|tool| tool.as_str().is_none_or(unsafe_tool))
                        }))
                    || unsafe_settings(value)
            }),
            Value::Array(values) => values.iter().any(unsafe_settings),
            _ => false,
        }
    }
    if config.settings.as_ref().is_some_and(unsafe_settings)
        || config.agents.as_ref().is_some_and(unsafe_settings)
        || config.allowed_tools_list.iter().any(|t| unsafe_tool(t))
        || config.env.keys().any(|key| {
            key.starts_with("CLAUDE") || key == "NODE_OPTIONS" || key == "BASH_ENV" || key == "ENV"
        })
    {
        anyhow::bail!(
            "Worker settings conflict with managed Claude MCP isolation; remove MCP, permission, hook, plugin or runtime overrides"
        );
    }
    Ok(())
}

// Stateless Streamable HTTP, tied to the execution's opaque capabilities. No
// upstream credentials or raw configurations are accessible through this API.
async fn serve_broker(
    listener: tokio::net::TcpListener,
    sessions: &Sessions,
    token: &str,
) -> anyhow::Result<()> {
    use futures_util::{StreamExt, stream::FuturesUnordered};
    let mut requests = FuturesUnordered::new();
    loop {
        tokio::select! {
            socket = listener.accept(), if requests.len() < 8 => {
                let (socket, _) = socket?;
                requests.push(async move {
                    let service = service_fn(|request| broker_request(request, sessions, token));
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(35), hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(socket), service)).await;
                });
            }
            _ = requests.next(), if !requests.is_empty() => {}
        }
    }
}

async fn broker_request(
    request: Request<Incoming>,
    sessions: &Sessions,
    token: &str,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let response = |status, value: Value| {
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(value.to_string())))
            .unwrap()
    };
    if request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        != Some(format!("Bearer {token}").as_str())
    {
        return Ok(response(401, Value::Null));
    }
    if request.method() != hyper::Method::POST {
        return Ok(response(405, Value::Null));
    }
    let Some(connection) = request
        .uri()
        .path()
        .strip_prefix('/')
        .and_then(|s| s.parse::<usize>().ok())
        .and_then(|i| sessions.0.get(i))
    else {
        return Ok(response(404, Value::Null));
    };
    let body = Limited::new(request.into_body(), 1024 * 1024)
        .collect()
        .await;
    let Ok(body) = body else {
        return Ok(response(413, Value::Null));
    };
    let Ok(value) = serde_json::from_slice::<Value>(&body.to_bytes()) else {
        return Ok(response(400, Value::Null));
    };
    if value.get("id").is_none() {
        return Ok(Response::builder()
            .status(202)
            .body(Full::new(Bytes::new()))
            .unwrap());
    }
    let result = match value["method"].as_str().unwrap_or("") {
        "initialize" => {
            let requested = value["params"]["protocolVersion"].as_str().unwrap_or("");
            let version = if ["2025-03-26", "2025-06-18", "2025-11-25"].contains(&requested) {
                requested
            } else {
                "2025-03-26"
            };
            Ok(
                serde_json::json!({"protocolVersion":version, "capabilities":{"tools":{}}, "serverInfo":{"name":"orchestra-broker", "version":"1"}}),
            )
        }
        "ping" => Ok(serde_json::json!({})),
        "tools/list" => connection.tools_list().await,
        "tools/call" => {
            connection
                .tools_call(
                    value["params"]["name"].as_str().unwrap_or(""),
                    value["params"]["arguments"].clone(),
                )
                .await
        }
        _ => Err(diraigent_orchestra::mcp::BrokerError::Denied),
    };
    let reply = match result {
        Ok(result) => serde_json::json!({"jsonrpc":"2.0", "id":value["id"], "result":result}),
        Err(error) => {
            serde_json::json!({"jsonrpc":"2.0", "id":value["id"], "error":{"code":-32603, "message":error.to_string()}})
        }
    };
    Ok(response(200, reply))
}

/// Parse the stream-json log file to extract cost, tokens, turns, result text, and error info.
///
/// Returns `(cost, input_tokens, output_tokens, turns, stop_reason, is_error, result_text)`.
pub(crate) async fn parse_result_from_log(
    log_file: &Path,
) -> (f64, u64, u64, u64, String, bool, String) {
    let content = match tokio::fs::read_to_string(log_file).await {
        Ok(c) => c,
        Err(e) => {
            warn!("could not read log file {}: {e}", log_file.display());
            return (0.0, 0, 0, 0, "unknown".into(), false, String::new());
        }
    };

    // Find last result line
    let result_line = content
        .lines()
        .rev()
        .find(|l| l.contains("\"type\":\"result\""));

    let Some(line) = result_line else {
        warn!("no result line found in log {}", log_file.display());
        return (0.0, 0, 0, 0, "unknown".into(), false, String::new());
    };

    // Try to parse the JSON (the line may have extra characters from script)
    let json_start = line.find('{');
    let Some(start) = json_start else {
        return (0.0, 0, 0, 0, "unknown".into(), false, String::new());
    };

    let json_str = &line[start..];
    let parsed: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return (0.0, 0, 0, 0, "unknown".into(), false, String::new()),
    };

    let cost = parsed["total_cost_usd"].as_f64().unwrap_or(0.0);
    let turns = parsed["num_turns"].as_u64().unwrap_or(0);
    let stop = parsed["stop_reason"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let is_error = parsed["is_error"].as_bool().unwrap_or(false);
    let result_text = parsed["result"].as_str().unwrap_or("").to_string();

    // Sum all input token variants (regular + cache creation + cache read).
    let usage = &parsed["usage"];
    let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0)
        + usage["cache_creation_input_tokens"].as_u64().unwrap_or(0)
        + usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
    let output_tokens = usage["output_tokens"].as_u64().unwrap_or(0);

    (
        cost,
        input_tokens,
        output_tokens,
        turns,
        stop,
        is_error,
        result_text,
    )
}

#[cfg(test)]
#[path = "../../tests/fixtures/mcp_claude_code.rs"]
mod mcp_tests;
