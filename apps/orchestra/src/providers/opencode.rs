use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, bail};
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use super::{ProviderConfig, ResolvedTask, TaskContext, TaskOutput, TaskProvider};

pub struct OpenCodeProvider;

/// V2 uses a private server and `--auto`; V1 uses the legacy permission flag.
pub(crate) async fn run_command(binary: &Path) -> anyhow::Result<Command> {
    let help = Command::new(binary)
        .args(["run", "--help"])
        .output()
        .await
        .context("inspect OpenCode CLI options")?;
    let options = String::from_utf8_lossy(&help.stdout);
    let mut command = Command::new(binary);
    command.args(["run", "--format", "json"]);
    if options.contains("--standalone") && options.contains("--auto") {
        command.args(["--standalone", "--auto"]);
    } else {
        command.arg("--dangerously-skip-permissions");
    }
    Ok(command)
}

#[async_trait]
impl TaskProvider for OpenCodeProvider {
    async fn execute(
        &self,
        step: &ResolvedTask,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<TaskOutput> {
        execute_with_binary(Path::new("opencode"), step, task, config).await
    }
}

async fn execute_with_binary(
    binary: &Path,
    step: &ResolvedTask,
    task: &TaskContext,
    config: &ProviderConfig,
) -> anyhow::Result<TaskOutput> {
    let worktree = task
        .working_dir
        .as_deref()
        .context("OpenCode requires working_dir")?;
    let log_file = task
        .log_file
        .as_deref()
        .context("OpenCode requires log_file")?;
    let agent = match step.allowed_tools.as_deref() {
        Some("readonly") => "plan",
        Some("full" | "merge") | None => "build",
        Some(other) => bail!("unsupported OpenCode tool preset: {other}"),
    };
    let user_prompt = task.user_prompt.as_deref().unwrap_or(&task.project_context);
    let prompt = match step.system_prompt.as_deref() {
        Some(system) if !system.is_empty() => format!("{system}\n\n{user_prompt}"),
        _ => user_prompt.to_string(),
    };

    let mut command = run_command(binary).await?;
    command
        .arg("--agent")
        .arg(agent)
        .current_dir(worktree)
        .envs(&step.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let worker_model = std::env::var("OPENCODE_MODEL")
        .ok()
        .filter(|s| !s.is_empty());
    if let Some(model) = step
        .model
        .as_deref()
        .or(config.model.as_deref())
        .or(worker_model.as_deref())
    {
        command.arg("--model").arg(model);
    }
    command.arg(prompt);

    let output = command
        .spawn()
        .context("start OpenCode CLI; install and authenticate `opencode` on the orchestra host")?
        .wait_with_output()
        .await
        .context("wait for OpenCode CLI")?;

    if let Some(parent) = log_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(log_file, &output.stdout)
        .await
        .context("write OpenCode event log")?;

    if !output.status.success() {
        bail!(
            "OpenCode CLI exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(1000)
                .collect::<String>()
        );
    }
    parse_events(&output.stdout)
}

fn parse_events(stdout: &[u8]) -> anyhow::Result<TaskOutput> {
    let mut parts: HashMap<String, String> = HashMap::new();
    let mut ordered = Vec::new();
    let mut input_tokens = 0;
    let mut output_tokens = 0;
    let mut cost_usd = 0.0;
    let mut turns = 0;
    let mut completed_text = false;

    for line in String::from_utf8_lossy(stdout).lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event["type"].as_str() {
            Some("text") => {
                if event["part"]["time"]["end"]
                    .as_u64()
                    .is_some_and(|end| end > 0)
                {
                    completed_text = true;
                }
                if let Some(value) = event["part"]["text"].as_str() {
                    let id = event["part"]["id"].as_str().unwrap_or("").to_string();
                    if !parts.contains_key(&id) {
                        ordered.push(id.clone());
                    }
                    parts.insert(id, value.to_string());
                }
            }
            Some("step_finish" | "step-finish") => {
                turns += 1;
                let part = &event["part"];
                input_tokens += part["tokens"]["input"].as_u64().unwrap_or(0);
                output_tokens += part["tokens"]["output"].as_u64().unwrap_or(0);
                cost_usd += part["cost"].as_f64().unwrap_or(0.0);
            }
            Some("error") => bail!("OpenCode failed: {}", event["error"]),
            _ => {}
        }
    }
    if turns == 0 && completed_text {
        turns = 1;
    }
    if turns == 0 {
        bail!("OpenCode CLI returned no completed step");
    }
    let content = ordered
        .iter()
        .filter_map(|id| parts.get(id))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    Ok(TaskOutput {
        content,
        exit_code: 0,
        artifacts: HashMap::new(),
        cost_usd,
        input_tokens,
        output_tokens,
        num_turns: turns,
        stop_reason: "completed".into(),
        is_error: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_and_usage() {
        let events = b"{\"type\":\"text\",\"part\":{\"id\":\"one\",\"text\":\"Done\"}}\n{\"type\":\"step_finish\",\"part\":{\"tokens\":{\"input\":12,\"output\":4},\"cost\":0.01}}\n";
        let result = parse_events(events).unwrap();
        assert_eq!(result.content, "Done");
        assert_eq!(result.input_tokens, 12);
        assert_eq!(result.output_tokens, 4);
    }

    #[test]
    fn rejects_incomplete_step() {
        assert!(parse_events(b"{\"type\":\"text\",\"part\":{\"text\":\"partial\"}}\n").is_err());
    }

    #[test]
    fn accepts_v2_completed_text_without_step_finish() {
        let result = parse_events(b"{\"type\":\"text\",\"part\":{\"id\":\"one\",\"text\":\"Done\",\"time\":{\"end\":1}}}\n").unwrap();
        assert_eq!(result.content, "Done");
        assert_eq!(result.num_turns, 1);
    }

    #[test]
    fn completed_text_does_not_double_count_v1_steps() {
        let result = parse_events(b"{\"type\":\"text\",\"part\":{\"text\":\"Done\",\"time\":{\"end\":1}}}\n{\"type\":\"step_finish\",\"part\":{}}\n").unwrap();
        assert_eq!(result.num_turns, 1);
    }
}
