use crate::api::ProjectsApi;
use crate::crypto::Dek;
use crate::git::WorktreeManager;
use crate::prompt;
use crate::providers::{
    ProviderConfig as ProviderCfg, ProviderFactory, ResolvedStep,
    TaskContext as ProviderTaskContext,
};
use crate::step_profile::StepProfile;
use crate::task_id::TaskId;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{error, info, warn};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Default allowed tools for implement/rework steps (full access).
const TOOLS_FULL: &[&str] = &[
    "Bash(*)",
    "Read",
    "Write",
    "Edit",
    "Glob",
    "Grep",
    "WebFetch",
    "WebSearch",
];

/// Read-only tools for review and dream steps (no code modification).
const TOOLS_READONLY: &[&str] = &["Bash(*)", "Read", "Glob", "Grep", "WebFetch", "WebSearch"];

/// Minimal tools for merge steps (just git operations).
const TOOLS_MERGE: &[&str] = &["Bash(*)", "Read", "Glob", "Grep"];

/// Per-step configuration for AI cost optimization.
///
/// Configuration is read from the playbook step JSONB with hardcoded
/// fallback defaults based on step name.
pub struct StepConfig {
    /// Model override for this step. `None` means use CLI default.
    pub model: Option<String>,
    /// Budget in USD for `--max-budget-usd`. `None` means unlimited.
    pub budget: Option<f64>,
    /// Which tools to allow for this step.
    pub allowed_tools: Vec<String>,
    /// The step name (for logging).
    pub step_name: String,
    /// The raw playbook step JSONB (for prompt context_level).
    pub step_json: Option<Value>,
    /// MCP server configurations. Must be a JSON object with a top-level
    /// `"mcpServers"` key, e.g.:
    /// `{"mcpServers": {"fs": {"command": "npx", "args": [...]}}}`.
    /// Written to a temp file and passed as `--mcp-config <file>`.
    pub mcp_servers: Option<Value>,
    /// Custom sub-agent definitions passed as `--agents <json>`.
    /// Each key is an agent name, value is `{description, prompt}`.
    pub agents: Option<Value>,
    /// Name of a configured agent to activate via `--agent <name>`.
    pub agent: Option<String>,
    /// Additional Claude settings (skills, keybindings, etc.) passed
    /// as `--settings <json>`. Merged with project settings at runtime.
    pub settings: Option<Value>,
    /// Extra environment variables injected into the worker's shell.
    /// Useful for MCP servers or tools that require API keys.
    pub env: HashMap<String, String>,
    /// AI provider for this step (e.g. "anthropic", "openai", "ollama").
    /// `None` defaults to "anthropic".
    pub provider: Option<String>,
    /// Override the default API endpoint for the chosen provider.
    pub base_url: Option<String>,
}

impl StepConfig {
    /// Resolve step-specific configuration.
    ///
    /// Model priority: task context > playbook step JSONB > hardcoded step default.
    /// Hardcoded defaults always apply (sonnet for all steps), so `env_model` is
    /// intentionally not used — per-step defaults are more specific than a global env override.
    pub fn for_step(
        step_name: &str,
        step_json: Option<&Value>,
        task_model: Option<&str>,
        _env_model: Option<&str>,
    ) -> Self {
        // 1. Hardcoded defaults based on step profile
        let profile = StepProfile::for_step(step_name);
        let (default_model, default_budget, default_tools) = match profile {
            StepProfile::Review => ("sonnet", 5.0, "readonly"),
            StepProfile::Merge => ("sonnet", 2.5, "merge"),
            StepProfile::Dream => ("sonnet", 4.0, "readonly"),
            StepProfile::Implement => ("sonnet", 12.0, "full"),
        };

        // 2. Read overrides from playbook step JSONB
        let step_model = step_json.and_then(|s| s["model"].as_str());
        let step_budget = step_json.and_then(|s| s["budget"].as_f64());
        let step_tools = step_json.and_then(|s| s["allowed_tools"].as_str());

        // 3. Model: task > step JSON > hardcoded step default
        let model = task_model
            .map(|m| m.to_string())
            .or_else(|| step_model.map(|m| m.to_string()))
            .or_else(|| Some(default_model.to_string()));

        // 4. Budget: step JSON > hardcoded
        let budget = Some(step_budget.unwrap_or(default_budget));

        // 5. Tools: step JSON preset > hardcoded preset
        let tools_preset = step_tools.unwrap_or(default_tools);
        let allowed_tools = tools_for_preset(tools_preset);

        // 6. MCP servers: written to temp file, passed as --mcp-config
        let mcp_servers = step_json.and_then(|s| s.get("mcp_servers").cloned());

        // 7. Custom sub-agents: passed as --agents '<json>'
        let agents = step_json.and_then(|s| s.get("agents").cloned());

        // 8. Specific agent to activate: passed as --agent <name>
        let agent = step_json.and_then(|s| s["agent"].as_str().map(String::from));

        // 9. Additional Claude settings (skills, etc.): passed as --settings '<json>'
        let settings = step_json.and_then(|s| s.get("settings").cloned());

        // 10. Extra env vars: exported in wrapper script before exec
        let env: HashMap<String, String> = step_json
            .and_then(|s| s.get("env"))
            .and_then(|e| e.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // 11. Provider: which AI provider to use (default: anthropic)
        let provider = step_json.and_then(|s| s["provider"].as_str().map(String::from));

        // 12. Base URL: override the default API endpoint for the provider
        let base_url = step_json.and_then(|s| s["base_url"].as_str().map(String::from));

        StepConfig {
            model,
            budget,
            allowed_tools,
            step_name: step_name.to_string(),
            step_json: step_json.cloned(),
            mcp_servers,
            agents,
            agent,
            settings,
            env,
            provider,
            base_url,
        }
    }
}

/// Map a tool preset string to the list of allowed tools.
fn tools_for_preset(preset: &str) -> Vec<String> {
    let tools: &[&str] = match preset {
        "readonly" => TOOLS_READONLY,
        "merge" => TOOLS_MERGE,
        _ => TOOLS_FULL,
    };
    tools.iter().map(|t| t.to_string()).collect()
}

/// Result of running a Claude Code worker.
pub struct WorkerResult {
    pub task_id: String,
    pub has_changes: bool,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_seconds: u64,
    pub api_turns: u64,
    pub stop_reason: String,
    pub is_error: bool,
}

/// Spawn a Claude Code worker for a task. Runs to completion.
#[allow(clippy::too_many_arguments)]
pub async fn run_worker(
    api: &ProjectsApi,
    worktree_mgr: &WorktreeManager,
    task_id: &str,
    project_id: &str,
    repo_root: &Path,
    agent_cli: &str,
    log_dir: &Path,
    step_config: &StepConfig,
    dek: Option<&Dek>,
    upload_logs: bool,
) -> Result<WorkerResult> {
    let tid = TaskId::new(task_id);
    let branch_name = tid.branch_name();

    // Create worktree
    let worktree_path = worktree_mgr.create_worktree(task_id).map_err(|e| {
        error!("worker {tid}: worktree creation failed: {e:#}");
        e
    })?;
    info!(
        "worker {tid}: worktree ready at {}",
        worktree_path.display()
    );

    // Build static system prompt (cached by Anthropic across invocations)
    let system_prompt = prompt::build_static_system_prompt(repo_root);

    // Build dynamic user prompt (changes per task, trimmed by step type)
    let user_prompt = prompt::build_user_prompt(
        api,
        task_id,
        project_id,
        &worktree_path,
        repo_root,
        agent_cli,
        &step_config.step_name,
        step_config.step_json.as_ref(),
        dek,
    )
    .await;

    // Ensure log directory exists
    tokio::fs::create_dir_all(log_dir).await.ok();

    let log_file = log_dir.join(format!("{}.log", tid.worktree_dir_name()));

    // Log invocation with step config details
    let model_info = step_config.model.as_deref().unwrap_or("default");
    let budget_info = step_config
        .budget
        .map(|b| format!("${b:.1}"))
        .unwrap_or_else(|| "unlimited".into());
    let mcp_info = if step_config.mcp_servers.is_some() {
        " +mcp"
    } else {
        ""
    };
    let agents_info = if step_config.agents.is_some() {
        " +agents"
    } else {
        ""
    };
    let settings_info = if step_config.settings.is_some() {
        " +settings"
    } else {
        ""
    };
    let env_info = if !step_config.env.is_empty() {
        format!(" +env({})", step_config.env.len())
    } else {
        String::new()
    };
    info!(
        "worker {tid}: invoking claude (step={}, model={model_info}, budget={budget_info}, tools={}{}{}{}{})",
        step_config.step_name,
        step_config.allowed_tools.len(),
        mcp_info,
        agents_info,
        settings_info,
        env_info,
    );

    // Route to the correct provider based on step config.
    // Default to "anthropic" (existing Claude Code subprocess path) when no
    // provider is specified, preserving backward compatibility.
    let provider_name = step_config.provider.as_deref().unwrap_or("anthropic");
    let start = std::time::Instant::now();

    let (result, cost_usd, input_tokens, output_tokens, api_turns, stop_reason, is_error) =
        if provider_name == "anthropic" {
            // Existing Claude Code subprocess path (unchanged behavior)
            let result = run_claude(
                &system_prompt,
                &user_prompt,
                &worktree_path,
                &log_file,
                step_config,
            )
            .await;
            let (cost, inp, out, turns, stop, err) = parse_result_from_log(&log_file).await;
            (result, cost, inp, out, turns, stop, err)
        } else {
            // Non-Anthropic provider routing
            execute_via_provider(
                api,
                provider_name,
                project_id,
                task_id,
                step_config,
                &system_prompt,
                &user_prompt,
            )
            .await
        };

    let duration = start.elapsed();

    // Commit changes
    let has_changes = worktree_mgr
        .commit_changes(&worktree_path, task_id)
        .unwrap_or(false);

    // Collect and post changed files (always check branch diff, not just uncommitted changes —
    // the agent commits its own changes during the run, so commit_changes may find nothing)
    match worktree_mgr.collect_changed_files(task_id) {
        Ok(files) if !files.is_empty() => {
            info!("worker {tid}: posting {} changed files", files.len());
            if let Err(e) = api.post_changed_files(task_id, &files).await {
                warn!("worker {tid}: failed to post changed files: {e}");
            }
        }
        Ok(_) => {
            info!("worker {tid}: no changed files found in branch diff");
        }
        Err(e) => warn!("worker {tid}: failed to collect changed files: {e}"),
    }

    // Scope violation detection: for implement steps, warn if the diff has far more
    // deletions than insertions — this typically means the agent used Write (full rewrite)
    // instead of Edit (targeted diff), causing collateral deletion of unrelated code.
    //
    // Thresholds: deletions > 50 AND deletions > 3× insertions is a strong signal.
    let is_retriable = step_config
        .step_json
        .as_ref()
        .map(crate::step_profile::is_retriable)
        .unwrap_or_else(|| {
            matches!(
                StepProfile::for_step(&step_config.step_name),
                StepProfile::Implement
            )
        });
    if is_retriable {
        match worktree_mgr.diff_insertion_deletion_stats(task_id) {
            Ok((insertions, deletions)) if deletions > 50 && deletions > insertions * 3 => {
                warn!(
                    "worker {tid}: scope violation suspected — {deletions} deletions vs {insertions} insertions"
                );
                let obs = serde_json::json!({
                    "kind": "risk",
                    "title": "Implement step has large-scale deletions — possible Write-tool collateral damage",
                    "description": format!(
                        "Task {tid} implement step deleted {deletions} lines but only added {insertions} lines \
                         (ratio {:.1}×). This is a strong indicator the agent used the Write tool to rewrite \
                         entire files instead of Edit for targeted changes, causing unrelated code to be silently \
                         removed. Review the diff carefully before merging: `git diff main...HEAD` in the worktree.",
                        if insertions == 0 { deletions as f64 } else { deletions as f64 / insertions as f64 }
                    ),
                    "severity": "high",
                    "source": "worker",
                    "source_task_id": task_id,
                });
                if let Err(e) = api.post_observation(project_id, &obs).await {
                    warn!("worker {tid}: failed to post scope violation observation: {e}");
                }
            }
            Ok((insertions, deletions)) => {
                info!("worker {tid}: diff stats: +{insertions} -{deletions} (scope ok)");
            }
            Err(e) => warn!("worker {tid}: failed to get diff stats: {e}"),
        }
    }

    // Post audit event
    let severity = if is_error { "error" } else { "info" };
    let title = if is_error {
        format!("Worker error: task {tid} (stop_reason={stop_reason})")
    } else {
        format!("Worker completed: task {tid}")
    };
    post_worker_event(
        api,
        project_id,
        task_id,
        &title,
        severity,
        serde_json::json!({
            "cost_usd": cost_usd,
            "duration_seconds": duration.as_secs(),
            "api_turns": api_turns,
            "stop_reason": &stop_reason,
            "is_error": is_error,
            "has_changes": has_changes,
            "branch": &branch_name,
            "step": &step_config.step_name,
            "model": model_info,
            "budget_usd": step_config.budget,
        }),
    )
    .await;

    if is_error {
        error!(
            "worker {tid} error: stop_reason={stop_reason} cost=${cost_usd:.2} turns={api_turns} {:.0}s",
            duration.as_secs_f64()
        );
    } else {
        info!(
            "done {tid} ${cost_usd:.2} turns={api_turns} {:.0}s",
            duration.as_secs_f64()
        );
    }

    if let Err(e) = result {
        error!("worker {tid}: claude process failed: {e:#}");
    }

    // Post cost metrics to the API so the project dashboard can show spend.
    if cost_usd > 0.0 || input_tokens > 0 || output_tokens > 0 {
        if let Err(e) = api
            .post_task_cost(task_id, input_tokens as i64, output_tokens as i64, cost_usd)
            .await
        {
            warn!("worker {tid}: failed to post cost metrics: {e}");
        } else {
            info!("worker {tid}: recorded cost  ({input_tokens} in / {output_tokens} out tokens)");
        }
    }

    // Upload task execution log to the API when the project has upload_logs enabled.
    if upload_logs {
        match tokio::fs::read_to_string(&log_file).await {
            Ok(content) if !content.is_empty() => {
                let log_metadata = serde_json::json!({
                    "cost_usd": cost_usd,
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                    "api_turns": api_turns,
                    "stop_reason": &stop_reason,
                    "is_error": is_error,
                    "duration_seconds": duration.as_secs(),
                    "model": model_info,
                });
                if let Err(e) = api
                    .upload_task_log(
                        project_id,
                        task_id,
                        &step_config.step_name,
                        &content,
                        &log_metadata,
                    )
                    .await
                {
                    warn!("worker {tid}: failed to upload task log: {e}");
                } else {
                    info!("worker {tid}: uploaded task log to API");
                }
            }
            Ok(_) => {
                info!("worker {tid}: log file empty, skipping upload");
            }
            Err(e) => {
                warn!("worker {tid}: failed to read log for upload: {e}");
            }
        }
    }

    Ok(WorkerResult {
        task_id: task_id.to_string(),
        has_changes,
        cost_usd,
        input_tokens,
        output_tokens,
        duration_seconds: duration.as_secs(),
        api_turns,
        stop_reason,
        is_error,
    })
}

/// Build and post a structured worker event to the API.
///
/// Constructs a `custom` event with the standard `orchestra` source, the given
/// title, severity, task and agent identifiers, and an arbitrary metadata
/// object.  Any posting failure is logged as a warning so callers do not need
/// to handle the error themselves.
pub async fn post_worker_event(
    api: &ProjectsApi,
    project_id: &str,
    task_id: &str,
    title: &str,
    severity: &str,
    metadata: serde_json::Value,
) {
    let event = serde_json::json!({
        "kind": "custom",
        "source": "orchestra",
        "title": title,
        "severity": severity,
        "related_task_id": task_id,
        "agent_id": api.agent_id(),
        "metadata": metadata
    });
    if let Err(e) = api.post_event(project_id, &event).await {
        warn!("failed to post worker event: {e}");
    }
}

/// Execute a step via a non-Anthropic provider (OpenAI, Ollama, etc.).
///
/// This function:
/// 1. Creates the provider instance via [`ProviderFactory`].
/// 2. Resolves credentials from the API (project-level, then global fallback).
/// 3. Merges step-level overrides — `step.base_url` overrides config `base_url`,
///    explicit step/task `model` overrides config `default_model`.
/// 4. Calls `provider.execute()` and translates errors into blocker posts
///    rather than panicking.
///
/// Returns the same tuple shape as the Anthropic path so callers can share
/// post-processing (commit, audit events, cost metrics).
async fn execute_via_provider(
    api: &ProjectsApi,
    provider_name: &str,
    project_id: &str,
    task_id: &str,
    step_config: &StepConfig,
    _system_prompt: &str,
    user_prompt: &str,
) -> (Result<()>, f64, u64, u64, u64, String, bool) {
    let tid = TaskId::new(task_id);

    // 1. Create provider instance via factory
    let provider: Box<dyn crate::providers::StepProvider> =
        match ProviderFactory::create(provider_name) {
            Ok(p) => p,
            Err(e) => {
                let msg = format!("Unknown provider '{provider_name}': {e}");
                warn!("worker {tid}: {msg}");
                if let Err(be) = api.post_task_update(task_id, "blocker", &msg).await {
                    warn!("worker {tid}: failed to post blocker: {be}");
                }
                return (
                    Err(anyhow::anyhow!(msg)),
                    0.0,
                    0,
                    0,
                    0,
                    "provider_error".into(),
                    true,
                );
            }
        };

    // 2. Resolve credentials from API (project-level, then global fallback)
    let resolved_cfg = api.resolve_provider_config(project_id, provider_name).await;

    // 3. Build ProviderConfig with step-level overrides.
    //
    // For the model field we use the *explicit* step-JSON model (if set) rather
    // than `step_config.model`, because the latter includes a hardcoded
    // fallback ("sonnet") that is Anthropic-specific and meaningless for other
    // providers.  The explicit step model is the one the playbook author or
    // task creator intentionally set.
    let explicit_step_model = step_config
        .step_json
        .as_ref()
        .and_then(|s| s["model"].as_str())
        .map(String::from);

    let provider_cfg = match resolved_cfg {
        Ok(cfg) => ProviderCfg {
            api_key: cfg["api_key"].as_str().map(String::from),
            base_url: step_config
                .base_url
                .clone()
                .or_else(|| cfg["base_url"].as_str().map(String::from)),
            model: explicit_step_model.or_else(|| cfg["default_model"].as_str().map(String::from)),
        },
        Err(e) => {
            // No stored config — use step-level overrides only.
            // This is not fatal: the provider may work without credentials
            // (e.g. local Ollama).
            warn!("worker {tid}: no provider config for '{provider_name}': {e}");
            ProviderCfg {
                api_key: None,
                base_url: step_config.base_url.clone(),
                model: explicit_step_model,
            }
        }
    };

    // 4. Build resolved step from step config
    let step_description = step_config
        .step_json
        .as_ref()
        .and_then(|s| s["description"].as_str())
        .unwrap_or(&step_config.step_name)
        .to_string();

    let step = ResolvedStep {
        name: step_config.step_name.clone(),
        description: step_description,
        model: provider_cfg.model.clone(),
        allowed_tools: step_config
            .step_json
            .as_ref()
            .and_then(|s| s["allowed_tools"].as_str())
            .map(String::from),
        budget: step_config.budget,
        env: step_config.env.clone(),
    };

    // 5. Build task context for the provider
    let task_ctx = ProviderTaskContext {
        task_id: task_id.to_string(),
        project_id: project_id.to_string(),
        project_context: user_prompt.to_string(),
        previous_step_output: None,
    };

    // 6. Execute via provider — errors become blockers, not panics
    info!(
        "worker {tid}: executing via provider '{provider_name}' (model={}, base_url={})",
        provider_cfg.model.as_deref().unwrap_or("default"),
        provider_cfg.base_url.as_deref().unwrap_or("default"),
    );
    match provider.execute(&step, &task_ctx, &provider_cfg).await {
        Ok(output) => {
            let is_err = output.exit_code != 0;
            let stop = if is_err {
                "error".to_string()
            } else {
                "end_turn".to_string()
            };
            info!(
                "worker {tid}: provider '{provider_name}' completed (exit_code={})",
                output.exit_code
            );
            (Ok(()), 0.0, 0, 0, 0, stop, is_err)
        }
        Err(e) => {
            let msg = format!("Provider '{provider_name}' execution failed: {e:#}");
            warn!("worker {tid}: {msg}");
            if let Err(be) = api.post_task_update(task_id, "blocker", &msg).await {
                warn!("worker {tid}: failed to post blocker: {be}");
            }
            (
                Err(anyhow::anyhow!(msg)),
                0.0,
                0,
                0,
                0,
                "provider_error".into(),
                true,
            )
        }
    }
}

async fn run_claude(
    system_prompt: &str,
    user_prompt: &str,
    worktree: &Path,
    log_file: &Path,
    config: &StepConfig,
) -> Result<()> {
    // Write prompts to temp files to avoid OS ARG_MAX limits.
    // The user prompt includes full project context JSON and can exceed 256KB.
    // Previously, both prompts were passed as argv to `script` → `claude`,
    // which would fail with E2BIG when total argv exceeded ~1MB (macOS)
    // or ~2MB (Linux).
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

    // Build --allowedTools flags (small, safe for argv)
    let mut tool_flags = String::new();
    for tool in &config.allowed_tools {
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

    // Write MCP config to a temp file if mcp_servers is specified.
    // Passed as --mcp-config <path> to the claude command.
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
        // Escape single quotes in JSON for safe embedding in bash single-quoted string
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

    // Pass additional settings (skills, keybindings, etc.) as --settings '<json>'.
    let settings_flag = if let Some(settings) = &config.settings {
        let settings_str = serde_json::to_string(settings).unwrap_or_default();
        let escaped = settings_str.replace('\'', "'\\''");
        format!(" --settings '{escaped}'")
    } else {
        String::new()
    };

    // Create a wrapper script that pipes the user prompt via stdin.
    // This avoids putting the large user prompt in execve() argv.
    // The system prompt is read from file into a bash variable and
    // passed as --system-prompt (small, ~3KB from CLAUDE.md).
    //
    // `script` wraps everything in a PTY for Node.js output flushing,
    // but the inner pipe (cat | claude) keeps claude's stdin as a pipe
    // so `claude -p` reads the prompt from stdin.
    //
    // Extra env vars from the step config are passed directly to the child
    // process via Command::env() — never embedded in the shell script —
    // to prevent shell injection via maliciously crafted values.
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
    // macOS BSD: `script -q logfile bash wrapper.sh`
    // Linux:     `script -q -c "bash wrapper.sh" logfile`
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
        // Inject extra env vars from step config directly into the child process
        // environment — no shell involved, so no injection risk.
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

async fn parse_result_from_log(log_file: &Path) -> (f64, u64, u64, u64, String, bool) {
    let content = match tokio::fs::read_to_string(log_file).await {
        Ok(c) => c,
        Err(e) => {
            warn!("could not read log file {}: {e}", log_file.display());
            return (0.0, 0, 0, 0, "unknown".into(), false);
        }
    };

    // Find last result line
    let result_line = content
        .lines()
        .rev()
        .find(|l| l.contains("\"type\":\"result\""));

    let Some(line) = result_line else {
        warn!("no result line found in log {}", log_file.display());
        return (0.0, 0, 0, 0, "unknown".into(), false);
    };

    // Try to parse the JSON (the line may have extra characters from script)
    let json_start = line.find('{');
    let Some(start) = json_start else {
        return (0.0, 0, 0, 0, "unknown".into(), false);
    };

    let json_str = &line[start..];
    let parsed: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return (0.0, 0, 0, 0, "unknown".into(), false),
    };

    let cost = parsed["total_cost_usd"].as_f64().unwrap_or(0.0);
    let turns = parsed["num_turns"].as_u64().unwrap_or(0);
    let stop = parsed["stop_reason"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let is_error = parsed["is_error"].as_bool().unwrap_or(false);

    // Sum all input token variants (regular + cache creation + cache read).
    let usage = &parsed["usage"];
    let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0)
        + usage["cache_creation_input_tokens"].as_u64().unwrap_or(0)
        + usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
    let output_tokens = usage["output_tokens"].as_u64().unwrap_or(0);

    (cost, input_tokens, output_tokens, turns, stop, is_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ProjectsApi;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ── Provider routing decision tests ──────────────────────

    #[test]
    fn default_provider_is_anthropic() {
        let step_config = StepConfig::for_step("implement", None, None, None);
        let provider_name = step_config.provider.as_deref().unwrap_or("anthropic");
        assert_eq!(provider_name, "anthropic");
    }

    #[test]
    fn explicit_anthropic_provider_is_recognised() {
        let step_json = serde_json::json!({"name": "implement", "provider": "anthropic"});
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);
        let provider_name = step_config.provider.as_deref().unwrap_or("anthropic");
        assert_eq!(provider_name, "anthropic");
    }

    #[test]
    fn openai_provider_is_extracted_from_step_json() {
        let step_json = serde_json::json!({"name": "implement", "provider": "openai"});
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);
        assert_eq!(step_config.provider.as_deref(), Some("openai"));
    }

    #[test]
    fn ollama_provider_is_extracted_from_step_json() {
        let step_json = serde_json::json!({"name": "implement", "provider": "ollama"});
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);
        assert_eq!(step_config.provider.as_deref(), Some("ollama"));
    }

    #[test]
    fn base_url_extracted_from_step_json() {
        let step_json = serde_json::json!({
            "name": "implement",
            "provider": "openai",
            "base_url": "https://my-proxy.example.com"
        });
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);
        assert_eq!(
            step_config.base_url.as_deref(),
            Some("https://my-proxy.example.com")
        );
    }

    // ── End-to-end provider routing tests ────────────────────

    #[tokio::test]
    async fn routing_openai_executes_via_provider() {
        let server = MockServer::start().await;

        // Mock resolve endpoint returning OpenAI config
        Mock::given(method("GET"))
            .and(path("/proj-1/providers/resolve/openai"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "provider": "openai",
                "api_key": "sk-test-key",
                "base_url": "https://api.openai.com",
                "default_model": "gpt-4o",
                "api_key_source": "global"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let step_json = serde_json::json!({"name": "implement", "provider": "openai"});
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);

        let (result, cost, inp, out, turns, stop_reason, is_error) = execute_via_provider(
            &api,
            "openai",
            "proj-1",
            "task-1",
            &step_config,
            "system prompt",
            "user prompt",
        )
        .await;

        assert!(result.is_ok(), "openai provider execution should succeed");
        assert!(!is_error);
        assert_eq!(stop_reason, "end_turn");
        assert_eq!(cost, 0.0);
        assert_eq!(inp, 0);
        assert_eq!(out, 0);
        assert_eq!(turns, 0);
    }

    #[tokio::test]
    async fn routing_ollama_executes_via_provider() {
        let server = MockServer::start().await;

        // Mock resolve endpoint returning Ollama config
        Mock::given(method("GET"))
            .and(path("/proj-1/providers/resolve/ollama"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "provider": "ollama",
                "api_key": null,
                "base_url": "http://localhost:11434",
                "default_model": "llama3",
                "api_key_source": null
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let step_json = serde_json::json!({"name": "implement", "provider": "ollama"});
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);

        let (result, _, _, _, _, stop_reason, is_error) = execute_via_provider(
            &api,
            "ollama",
            "proj-1",
            "task-1",
            &step_config,
            "system prompt",
            "user prompt",
        )
        .await;

        assert!(result.is_ok(), "ollama provider execution should succeed");
        assert!(!is_error);
        assert_eq!(stop_reason, "end_turn");
    }

    #[tokio::test]
    async fn routing_anthropic_via_factory_executes_via_provider() {
        let server = MockServer::start().await;

        // Mock resolve endpoint returning Anthropic config
        Mock::given(method("GET"))
            .and(path("/proj-1/providers/resolve/anthropic"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "provider": "anthropic",
                "api_key": "sk-ant-test",
                "base_url": null,
                "default_model": "claude-sonnet-4-6",
                "api_key_source": "project"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let step_config = StepConfig::for_step("implement", None, None, None);

        // When called directly (not through run_worker routing), the anthropic
        // stub provider should also work via execute_via_provider.
        let (result, _, _, _, _, stop_reason, is_error) = execute_via_provider(
            &api,
            "anthropic",
            "proj-1",
            "task-1",
            &step_config,
            "system prompt",
            "user prompt",
        )
        .await;

        assert!(
            result.is_ok(),
            "anthropic provider execution should succeed"
        );
        assert!(!is_error);
        assert_eq!(stop_reason, "end_turn");
    }

    #[tokio::test]
    async fn unknown_provider_posts_blocker_and_returns_error() {
        let server = MockServer::start().await;

        // Mock blocker posting endpoint
        Mock::given(method("POST"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let step_config = StepConfig::for_step("implement", None, None, None);

        let (result, _, _, _, _, stop_reason, is_error) = execute_via_provider(
            &api,
            "foobar",
            "proj-1",
            "task-1",
            &step_config,
            "system prompt",
            "user prompt",
        )
        .await;

        assert!(result.is_err(), "unknown provider should return error");
        assert!(is_error);
        assert_eq!(stop_reason, "provider_error");
    }

    #[tokio::test]
    async fn step_level_base_url_overrides_provider_config() {
        let server = MockServer::start().await;

        // Provider config has default base_url
        Mock::given(method("GET"))
            .and(path("/proj-1/providers/resolve/openai"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "provider": "openai",
                "api_key": "sk-test",
                "base_url": "https://api.openai.com",
                "default_model": "gpt-4o",
                "api_key_source": "global"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");

        // Step config overrides base_url
        let step_json = serde_json::json!({
            "name": "implement",
            "provider": "openai",
            "base_url": "https://my-proxy.example.com"
        });
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);

        assert_eq!(
            step_config.base_url.as_deref(),
            Some("https://my-proxy.example.com"),
            "step-level base_url should be set"
        );

        let (result, _, _, _, _, _, is_error) = execute_via_provider(
            &api,
            "openai",
            "proj-1",
            "task-1",
            &step_config,
            "system",
            "user",
        )
        .await;

        assert!(result.is_ok());
        assert!(!is_error);
    }

    #[tokio::test]
    async fn step_level_model_overrides_provider_config_default_model() {
        let server = MockServer::start().await;

        // Provider config has default_model = "gpt-4o"
        Mock::given(method("GET"))
            .and(path("/proj-1/providers/resolve/openai"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "provider": "openai",
                "api_key": "sk-test",
                "base_url": null,
                "default_model": "gpt-4o",
                "api_key_source": "global"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");

        // Step config explicitly sets model to "gpt-4-turbo"
        let step_json = serde_json::json!({
            "name": "implement",
            "provider": "openai",
            "model": "gpt-4-turbo"
        });
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);

        let (result, _, _, _, _, _, is_error) = execute_via_provider(
            &api,
            "openai",
            "proj-1",
            "task-1",
            &step_config,
            "system",
            "user",
        )
        .await;

        assert!(result.is_ok());
        assert!(!is_error);
    }

    #[tokio::test]
    async fn provider_config_not_found_falls_back_to_step_overrides() {
        let server = MockServer::start().await;

        // Resolve endpoint returns 404 (no config stored)
        Mock::given(method("GET"))
            .and(path("/proj-1/providers/resolve/openai"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": "No provider config found for provider 'openai'"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");

        // Step config provides base_url and model as fallbacks
        let step_json = serde_json::json!({
            "name": "implement",
            "provider": "openai",
            "base_url": "https://fallback.example.com",
            "model": "gpt-4"
        });
        let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);

        // Should still succeed (provider stub doesn't need credentials)
        let (result, _, _, _, _, _, is_error) = execute_via_provider(
            &api,
            "openai",
            "proj-1",
            "task-1",
            &step_config,
            "system",
            "user",
        )
        .await;

        assert!(result.is_ok());
        assert!(!is_error);
    }

    #[tokio::test]
    async fn all_three_providers_route_correctly() {
        // End-to-end routing test covering all three providers
        let server = MockServer::start().await;

        // Mock resolve endpoint for all three providers
        for (provider, model) in [
            ("anthropic", "claude-sonnet-4-6"),
            ("openai", "gpt-4o"),
            ("ollama", "llama3"),
        ] {
            Mock::given(method("GET"))
                .and(path(format!("/proj-1/providers/resolve/{provider}")))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "provider": provider,
                    "api_key": "test-key",
                    "base_url": null,
                    "default_model": model,
                    "api_key_source": "global"
                })))
                .mount(&server)
                .await;
        }

        let api = ProjectsApi::new(&server.uri(), "test-agent");

        // Test each provider
        for provider_name in ["anthropic", "openai", "ollama"] {
            let step_json = serde_json::json!({
                "name": "implement",
                "provider": provider_name
            });
            let step_config = StepConfig::for_step("implement", Some(&step_json), None, None);

            let (result, _, _, _, _, stop_reason, is_error) = execute_via_provider(
                &api,
                provider_name,
                "proj-1",
                "task-1",
                &step_config,
                "system prompt",
                "user prompt",
            )
            .await;

            assert!(
                result.is_ok(),
                "provider '{provider_name}' should execute successfully"
            );
            assert!(
                !is_error,
                "provider '{provider_name}' should not report error"
            );
            assert_eq!(
                stop_reason, "end_turn",
                "provider '{provider_name}' should return end_turn"
            );
        }
    }
}
