//! Claude Code CLI provider — wraps the Claude Code CLI subprocess.
//!
//! Spawns `claude -p` in a PTY via `script`, reads the stream-json log file
//! for cost/token metrics, and returns a [`StepOutput`] with full telemetry.
//!
//! Registered as both `"claude-code"` (canonical) and `"anthropic"` (legacy alias).

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::Context;
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tracing::{error, warn};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::{ProviderConfig, ResolvedStep, StepOutput, StepProvider, TaskContext};

/// Provider that executes steps via the Claude Code CLI.
pub struct ClaudeCodeProvider;

#[async_trait]
impl StepProvider for ClaudeCodeProvider {
    async fn execute(
        &self,
        step: &ResolvedStep,
        task: &TaskContext,
        _config: &ProviderConfig,
    ) -> anyhow::Result<StepOutput> {
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

        Ok(StepOutput {
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
    config: &ResolvedStep,
) -> anyhow::Result<()> {
    // Write prompts to temp files to avoid OS ARG_MAX limits.
    let temp_name = log_file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("claude");
    let temp_dir = std::env::temp_dir().join(format!("claude-{temp_name}"));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .context("create temp dir for prompts")?;

    let prompt_file = temp_dir.join("prompt.txt");
    let system_file = temp_dir.join("system.txt");
    tokio::fs::write(&prompt_file, user_prompt)
        .await
        .context("write user prompt to temp file")?;
    tokio::fs::write(&system_file, system_prompt)
        .await
        .context("write system prompt to temp file")?;

    // Build --allowedTools flags
    let mut tool_flags = String::new();
    for tool in &config.allowed_tools_list {
        tool_flags.push_str(&format!(" --allowedTools '{tool}'"));
    }

    let model_flag = config
        .model
        .as_deref()
        .map(|m| format!(" --model '{m}'"))
        .unwrap_or_default();

    let budget_flag = config
        .budget
        .map(|b| format!(" --max-budget-usd {b:.1}"))
        .unwrap_or_default();

    // Write MCP config to a temp file if specified.
    let mcp_flag = if let Some(mcp) = &config.mcp_servers {
        let mcp_file = temp_dir.join("mcp_config.json");
        let mcp_json = serde_json::to_string_pretty(mcp).unwrap_or_default();
        tokio::fs::write(&mcp_file, &mcp_json)
            .await
            .context("write MCP config to temp file")?;
        format!(" --mcp-config '{}'", mcp_file.display())
    } else {
        String::new()
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
        .map(|a| format!(" --agent '{a}'"))
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
         SYSTEM=\"$(cat '{system}')\"\n\
         cat '{prompt}' | exec claude -p \\\n\
           --system-prompt \"$SYSTEM\" \\\n\
           --no-session-persistence \\\n\
           --dangerously-skip-permissions \\\n\
           --output-format stream-json \\\n\
           --verbose{model}{budget}{tools}{mcp}{agents}{agent}{settings}\n",
        system = system_file.display(),
        prompt = prompt_file.display(),
        model = model_flag,
        budget = budget_flag,
        tools = tool_flags,
        mcp = mcp_flag,
        agents = agents_flag,
        agent = agent_flag,
        settings = settings_flag,
    );

    let wrapper_path = temp_dir.join("run.sh");
    tokio::fs::write(&wrapper_path, &wrapper_content)
        .await
        .context("write wrapper script")?;

    #[cfg(unix)]
    std::fs::set_permissions(&wrapper_path, std::fs::Permissions::from_mode(0o755))
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
            "-c".to_string(),
            format!("bash {wrapper_str}"),
            log_path.to_string(),
        ]
    };

    let mut child = Command::new("script")
        .args(&script_args)
        .current_dir(worktree)
        .env_remove("CLAUDECODE")
        .envs(config.env.iter())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn script/claude process")?;

    let status = child.wait().await.context("wait for claude process")?;

    // Clean up temp files (best-effort)
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    if !status.success() {
        error!("claude exited with status {status}");
        anyhow::bail!("claude exited with status {status}");
    }
    Ok(())
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
