use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, bail};
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::{ProviderConfig, ResolvedTask, TaskContext, TaskOutput, TaskProvider};

pub struct CodexProvider;

#[async_trait]
impl TaskProvider for CodexProvider {
    async fn execute(
        &self,
        step: &ResolvedTask,
        task: &TaskContext,
        config: &ProviderConfig,
    ) -> anyhow::Result<TaskOutput> {
        execute_with_binary(Path::new("codex"), step, task, config).await
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
        .context("CodexProvider requires working_dir")?;
    let log_file = task
        .log_file
        .as_deref()
        .context("CodexProvider requires log_file")?;

    let sandbox = match step.allowed_tools.as_deref() {
        Some("readonly") => "read-only",
        Some("full" | "merge") => "workspace-write",
        Some(other) => bail!("unsupported Codex tool preset: {other}"),
        None => "workspace-write",
    };

    let mut command = Command::new(binary);
    command
        .arg("exec")
        .arg("--json")
        .arg("--ephemeral")
        .arg("--sandbox")
        .arg(sandbox)
        .current_dir(worktree)
        .envs(&step.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(model) = step.model.as_deref().or(config.model.as_deref()) {
        command.arg("--model").arg(model);
    }
    command.arg("-");

    let mut child = command
        .spawn()
        .context("start Codex CLI; install and authenticate `codex` on the orchestra host")?;
    let user_prompt = task.user_prompt.as_deref().unwrap_or(&task.project_context);
    let prompt = match step.system_prompt.as_deref() {
        Some(system) if !system.is_empty() => format!("{system}\n\n{user_prompt}"),
        _ => user_prompt.to_string(),
    };
    child
        .stdin
        .take()
        .context("open Codex stdin")?
        .write_all(prompt.as_bytes())
        .await
        .context("write Codex prompt")?;
    let output = child
        .wait_with_output()
        .await
        .context("wait for Codex CLI")?;

    if let Some(parent) = log_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(log_file, &output.stdout)
        .await
        .context("write Codex event log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Codex CLI exited with {}: {}",
            output.status,
            stderr.trim().chars().take(1000).collect::<String>()
        );
    }
    parse_events(&output.stdout)
}

fn parse_events(stdout: &[u8]) -> anyhow::Result<TaskOutput> {
    let mut content = None;
    let mut input_tokens = 0;
    let mut output_tokens = 0;
    let mut turns = 0;
    let mut completed = false;

    for line in String::from_utf8_lossy(stdout).lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event["type"].as_str() {
            Some("item.completed") if event["item"]["type"] == "agent_message" => {
                content = event["item"]["text"].as_str().map(str::to_string);
            }
            Some("turn.completed") => {
                completed = true;
                turns += 1;
                input_tokens += event["usage"]["input_tokens"].as_u64().unwrap_or(0);
                output_tokens += event["usage"]["output_tokens"].as_u64().unwrap_or(0);
            }
            Some("turn.failed") => bail!("Codex turn failed: {}", event),
            _ => {}
        }
    }

    if !completed {
        bail!("Codex CLI returned no completed turn");
    }
    Ok(TaskOutput {
        content: content.unwrap_or_default(),
        exit_code: 0,
        artifacts: HashMap::new(),
        cost_usd: 0.0,
        input_tokens,
        output_tokens,
        num_turns: turns,
        stop_reason: "completed".to_string(),
        is_error: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_result_and_usage() {
        let events = b"{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Done\"}}\n{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":12,\"output_tokens\":4}}\n";
        let result = parse_events(events).unwrap();
        assert_eq!(result.content, "Done");
        assert_eq!(result.input_tokens, 12);
        assert_eq!(result.output_tokens, 4);
        assert_eq!(result.num_turns, 1);
    }

    #[test]
    fn rejects_incomplete_turn() {
        assert!(parse_events(b"{\"type\":\"turn.started\"}\n").is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runs_cli_in_worktree_with_prompt_and_readonly_sandbox() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("fake-codex");
        let args_file = temp.path().join("args");
        let prompt_file = temp.path().join("prompt");
        let log_file = temp.path().join("events.jsonl");
        std::fs::write(
            &binary,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CODEX_TEST_ARGS\"\ncat > \"$CODEX_TEST_PROMPT\"\nprintf '%s\\n' '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"finished\"}}' '{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();

        let step = ResolvedTask {
            name: "review".into(),
            description: "Review".into(),
            model: Some("test-model".into()),
            allowed_tools: Some("readonly".into()),
            allowed_tools_list: vec![],
            budget: None,
            env: HashMap::from([
                ("CODEX_TEST_ARGS".into(), args_file.display().to_string()),
                (
                    "CODEX_TEST_PROMPT".into(),
                    prompt_file.display().to_string(),
                ),
            ]),
            system_prompt: Some("System instructions".into()),
            mcp_servers: None,
            agents: None,
            agent: None,
            settings: None,
        };
        let task = TaskContext {
            task_id: "task-1".into(),
            project_id: "project-1".into(),
            project_context: "User request".into(),
            working_dir: Some(temp.path().to_path_buf()),
            log_file: Some(log_file.clone()),
            user_prompt: None,
        };
        let config = ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };

        let result = execute_with_binary(&binary, &step, &task, &config)
            .await
            .unwrap();
        assert_eq!(result.content, "finished");
        assert_eq!(result.input_tokens, 3);
        assert_eq!(
            std::fs::read_to_string(prompt_file).unwrap(),
            "System instructions\n\nUser request"
        );
        let args = std::fs::read_to_string(args_file).unwrap();
        assert!(args.contains("--sandbox\nread-only\n"));
        assert!(args.contains("--model\ntest-model\n"));
        assert!(args.ends_with("-\n"));
        assert!(
            std::fs::read_to_string(log_file)
                .unwrap()
                .contains("turn.completed")
        );
    }
}
