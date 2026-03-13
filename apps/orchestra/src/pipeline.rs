//! Post-worker task state machine.
//!
//! After a worker finishes, this module checks the task's state and decides
//! whether to merge (all steps done), continue the pipeline (more steps),
//! or clean up (cancelled).

use anyhow::Result;
use serde_json::Value;
use tracing::{info, warn};

use crate::api::{ProjectsApi, retry_api_call};
use crate::constants::{
    STATE_BACKLOG, STATE_CANCELLED, STATE_DONE, STATE_HUMAN_REVIEW, STATE_READY,
};
use crate::git_strategy::{self, GitAction, GitStrategy};
use crate::step_profile;
use crate::task_id::TaskId;

/// Outcome of checking the post-worker task state.
#[derive(Debug, PartialEq)]
pub enum StepOutcome {
    /// Pipeline continues — task is ready for the next step. No merge needed.
    Continue,
    /// Pipeline continues, but a mid-pipeline git action is needed first.
    ContinueWithGitAction {
        project_id: String,
        git_strategy: GitStrategy,
        git_action: GitAction,
    },
    /// All pipeline steps completed (or no playbook).
    /// Carries the resolved git strategy and project_id for post-completion handling.
    AllDone {
        project_id: String,
        git_strategy: GitStrategy,
    },
    /// Task was cancelled. Clean up worktree without merging.
    Cancelled { project_id: String },
    /// Task is in human_review — no action needed, human is reviewing.
    AlreadyReady,
    /// Task is in an unexpected state — log a warning but do not merge.
    UnexpectedState(String),
}

impl StepOutcome {
    /// Whether the outcome requires worktree cleanup without merging.
    pub fn should_cleanup_worktree(&self) -> bool {
        matches!(self, StepOutcome::Cancelled { .. })
    }
}

/// Check the post-worker task state and decide whether to merge.
///
/// Pipeline advancement and step regression are now handled atomically
/// by the API's `transition_task()`. This function only needs to distinguish:
/// - `ready` → pipeline advanced or rejection handled by API → Continue
/// - `done` → all steps truly completed → AllDone (trigger merge-to-main)
/// - `cancelled` → task was cancelled mid-pipeline → Cancelled
/// - other → unexpected state → UnexpectedState
pub async fn check_next_step(api: &ProjectsApi, task_id: &str) -> Result<StepOutcome> {
    let tid = TaskId::new(task_id);
    let task = retry_api_call("get_task", &tid, || api.get_task(task_id)).await?;
    let state = task["state"].as_str().unwrap_or("");
    let project_id = task["project_id"].as_str().unwrap_or("").to_string();

    if state == STATE_READY {
        // Task was sent back to ready by the current step (e.g. review rejection).
        // If the current step is a non-implement step, regress playbook_step back
        // to the previous implement step so it can apply the feedback.
        let playbook_id = task["playbook_id"].as_str().unwrap_or("");
        let current_step = task["playbook_step"].as_i64().unwrap_or(0);
        if !playbook_id.is_empty() {
            let playbook =
                retry_api_call("get_playbook", &tid, || api.get_playbook(playbook_id)).await?;
            // Prefer resolved_steps (templates expanded) over raw steps
            let steps_value = playbook["resolved_steps"]
                .as_array()
                .or_else(|| playbook["steps"].as_array());
            if let Some(steps) = steps_value {
                // Check if the *completed* step (current_step - 1) has a git_action.
                // The API advanced playbook_step already, so the just-finished step is one back.
                if current_step > 0
                    && let Some(completed_step_json) = steps.get((current_step - 1) as usize)
                {
                    let git_action = GitAction::from_step_json(completed_step_json);
                    if git_action != GitAction::None {
                        let git_mode = if let Ok(project) = api.get_project(&project_id).await {
                            project["git_mode"]
                                .as_str()
                                .unwrap_or("standalone")
                                .to_string()
                        } else {
                            "standalone".to_string()
                        };
                        let strategy =
                            git_strategy::resolve_strategy(api, Some(&task), &git_mode).await;
                        let completed_name =
                            completed_step_json["name"].as_str().unwrap_or("unknown");
                        info!(
                            "task {tid} step {completed_name} has git_action={:?} — performing before next step",
                            git_action
                        );
                        return Ok(StepOutcome::ContinueWithGitAction {
                            project_id,
                            git_strategy: strategy,
                            git_action,
                        });
                    }
                }

                let current_json = steps.get(current_step as usize);
                let current_name = current_json.and_then(|s| s["name"].as_str()).unwrap_or("");
                let current_retriable =
                    current_json.map(step_profile::is_retriable).unwrap_or(true);
                if !current_retriable {
                    for prev in (0..current_step).rev() {
                        if let Some(prev_step) = steps.get(prev as usize)
                            && step_profile::is_retriable(prev_step)
                        {
                            let prev_name = prev_step["name"].as_str().unwrap_or("");
                            info!(
                                "task {tid} {current_name} rejected — regressing to step {prev} ({prev_name})"
                            );
                            let update_body = serde_json::json!({"playbook_step": prev});
                            retry_api_call("update_task", &tid, || {
                                api.update_task(task_id, &update_body)
                            })
                            .await?;
                            break;
                        }
                    }
                }
            }
        }
        return Ok(StepOutcome::Continue);
    }

    if state == STATE_DONE {
        // Resolve git strategy from playbook metadata for post-completion handling.
        let git_mode = if let Ok(project) = api.get_project(&project_id).await {
            project["git_mode"]
                .as_str()
                .unwrap_or("standalone")
                .to_string()
        } else {
            "standalone".to_string()
        };
        let strategy = git_strategy::resolve_strategy(api, Some(&task), &git_mode).await;
        info!(
            "task {tid} completed all playbook steps (git_strategy={})",
            strategy.id()
        );
        return Ok(StepOutcome::AllDone {
            project_id,
            git_strategy: strategy,
        });
    }

    if state.starts_with("wait:") {
        let next_name = state.strip_prefix("wait:").unwrap_or("unknown");
        info!("task {tid} waiting for next step: {next_name}");
        return Ok(StepOutcome::Continue);
    }

    if state == STATE_CANCELLED {
        info!("task {tid} was cancelled — will clean up worktree");
        return Ok(StepOutcome::Cancelled { project_id });
    }

    if state == STATE_HUMAN_REVIEW {
        info!("task {tid} is in human_review — no action needed");
        return Ok(StepOutcome::AlreadyReady);
    }

    if state == STATE_BACKLOG {
        info!("task {tid} is in backlog — no action needed");
        return Ok(StepOutcome::AlreadyReady);
    }

    warn!("task {tid} in unexpected state '{state}' — skipping merge");
    Ok(StepOutcome::UnexpectedState(state.to_string()))
}

/// Count the number of `blocker` updates posted on a task.
///
/// Used for loop detection: each failed implement cycle posts a blocker before
/// releasing the task back to `ready`.
pub async fn count_blocker_cycles(api: &ProjectsApi, task_id: &str) -> u32 {
    match api.get_task_updates(task_id).await {
        Ok(updates) => updates
            .iter()
            .filter(|u| u["kind"].as_str() == Some("blocker"))
            .filter(|u| u["agent_id"].as_str().is_some())
            .count() as u32,
        Err(e) => {
            warn!("loop-detect: failed to fetch updates for {task_id}: {e}");
            0
        }
    }
}

/// Resolve the effective max_implement_cycles for a task's project.
pub async fn resolve_max_implement_cycles(
    api: &ProjectsApi,
    project_id: &str,
    global_max: u32,
) -> u32 {
    match api.get_project(project_id).await {
        Ok(project) => project["metadata"]["max_implement_cycles"]
            .as_u64()
            .map(|v| v as u32)
            .unwrap_or(global_max),
        Err(e) => {
            warn!(
                "resolve_max_implement_cycles: failed to fetch project {project_id}: {e}, using global default"
            );
            global_max
        }
    }
}

/// Resolve the playbook step for a task.
///
/// Returns (step_name, step_json) where step_json is the full step object
/// from the playbook JSONB.
///
/// If the playbook response includes `resolved_steps` (meaning step templates
/// were expanded by the API), uses those. Otherwise falls back to raw `steps`.
/// If a step has a `step_template_id` but the API didn't resolve it (e.g. older
/// API version), the orchestra fetches the template directly and merges inline.
pub async fn resolve_step(api: &ProjectsApi, task_data: Option<&Value>) -> (String, Option<Value>) {
    let Some(task) = task_data else {
        return ("implement".to_string(), None);
    };

    let playbook_id = task["playbook_id"].as_str().unwrap_or("");
    let current_step = task["playbook_step"].as_u64().unwrap_or(0);

    if playbook_id.is_empty() {
        return ("implement".to_string(), None);
    }

    if let Ok(playbook) = api.get_playbook(playbook_id).await {
        // Prefer resolved_steps (templates already expanded by API) over raw steps
        let steps = playbook["resolved_steps"]
            .as_array()
            .or_else(|| playbook["steps"].as_array());

        if let Some(steps) = steps
            && let Some(step) = steps.get(current_step as usize)
        {
            // If the step still has an unresolved step_template_id (API didn't expand),
            // try to resolve it client-side as a fallback.
            let resolved = if step.get("step_template_id").is_some_and(|v| v.is_string())
                && playbook["resolved_steps"].is_null()
            {
                resolve_step_template(api, step).await
            } else {
                step.clone()
            };

            let name = resolved["name"].as_str().unwrap_or("implement").to_string();
            (name, Some(resolved))
        } else {
            ("implement".to_string(), None)
        }
    } else {
        ("implement".to_string(), None)
    }
}

/// Client-side fallback: resolve a single step's step_template_id by fetching
/// the template from the API and merging its properties as defaults.
/// Inline step properties take precedence over template defaults.
async fn resolve_step_template(api: &ProjectsApi, step: &Value) -> Value {
    let template_id = match step["step_template_id"].as_str() {
        Some(id) => id,
        None => return step.clone(),
    };

    match api.get_step_template(template_id).await {
        Ok(template) => {
            let mut merged = serde_json::Map::new();

            // Start with template values as defaults
            if let Some(obj) = template.as_object() {
                // Copy only step-relevant fields from the template
                for key in &[
                    "name",
                    "description",
                    "model",
                    "budget",
                    "allowed_tools",
                    "context_level",
                    "on_complete",
                    "retriable",
                    "max_cycles",
                    "timeout_minutes",
                    "mcp_servers",
                    "agents",
                    "agent",
                    "settings",
                    "env",
                    "vars",
                ] {
                    if let Some(v) = obj.get(*key)
                        && !v.is_null()
                    {
                        merged.insert(key.to_string(), v.clone());
                    }
                }
            }

            // Overlay inline step properties (inline wins)
            if let Some(obj) = step.as_object() {
                for (key, value) in obj {
                    if !value.is_null() {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }

            Value::Object(merged)
        }
        Err(e) => {
            warn!(
                "Failed to fetch step_template_id {}: {} — using inline properties only",
                template_id, e
            );
            step.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ProjectsApi;
    use wiremock::matchers::{body_json, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn standard_playbook() -> serde_json::Value {
        serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "implement", "step": 0},
                {"name": "review", "step": 1},
                {"name": "merge", "step": 2},
                {"name": "dream", "step": 3}
            ]
        })
    }

    fn task_json(state: &str, playbook_step: u64, playbook_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "task-1",
            "state": state,
            "playbook_id": playbook_id,
            "playbook_step": playbook_step
        })
    }

    fn test_api(base_url: &str) -> ProjectsApi {
        ProjectsApi::new(base_url, "test-agent")
    }

    #[tokio::test]
    async fn ready_at_review_step_regresses_to_implement() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 1, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/tasks/task-1"))
            .and(body_json(serde_json::json!({"playbook_step": 0})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn ready_at_implement_step_does_not_regress() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 0, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn check_next_step_get_playbook_failure_returns_err() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 1, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(500))
            .expect(4)
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn done_means_all_steps_completed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("done", 0, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path_regex("/tasks/.*/transition"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), StepOutcome::AllDone { .. }));
    }

    #[tokio::test]
    async fn wait_state_returns_ok() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json(
                "wait:review",
                1,
                "pb-1",
            )))
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert_eq!(result.unwrap(), StepOutcome::Continue);
    }

    #[tokio::test]
    async fn ready_at_review_step_get_playbook_fails_no_regression() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 1, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(500))
            .expect(4)
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn check_next_step_get_playbook_retries_transient_500() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 1, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("PUT"))
            .and(path("/tasks/task-1"))
            .and(body_json(serde_json::json!({"playbook_step": 0})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn done_is_always_terminal_in_orchestra() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("done", 0, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path_regex("/tasks/.*/transition"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), StepOutcome::AllDone { .. }));
    }

    #[tokio::test]
    async fn no_playbook_id_returns_ok() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 0, "")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path_regex("/playbooks/.*"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
    }

    // ── count_blocker_cycles tests ──

    #[tokio::test]
    async fn count_blocker_cycles_zero_updates_returns_zero() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        assert_eq!(count_blocker_cycles(&api, "task-1").await, 0);
    }

    #[tokio::test]
    async fn count_blocker_cycles_three_blockers_returns_three() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"kind": "blocker", "message": "build failed", "agent_id": "agent-1"},
                {"kind": "blocker", "message": "test failed", "agent_id": "agent-1"},
                {"kind": "blocker", "message": "lint failed", "agent_id": "agent-1"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        assert_eq!(count_blocker_cycles(&api, "task-1").await, 3);
    }

    #[tokio::test]
    async fn count_blocker_cycles_mixed_kinds_counts_only_blockers() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"kind": "progress", "message": "started", "agent_id": "agent-1"},
                {"kind": "blocker", "message": "build failed", "agent_id": "agent-1"},
                {"kind": "artifact", "message": "test output", "agent_id": "agent-1"},
                {"kind": "blocker", "message": "lint failed", "agent_id": "agent-1"},
                {"kind": "progress", "message": "retrying", "agent_id": "agent-1"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        assert_eq!(count_blocker_cycles(&api, "task-1").await, 2);
    }

    #[tokio::test]
    async fn count_blocker_cycles_excludes_human_posted_blockers() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"kind": "blocker", "message": "build failed", "agent_id": "agent-1"},
                {"kind": "blocker", "message": "waiting on design decision", "user_id": "user-1"},
                {"kind": "blocker", "message": "lint failed", "agent_id": "agent-1"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        assert_eq!(count_blocker_cycles(&api, "task-1").await, 2);
    }

    #[tokio::test]
    async fn count_blocker_cycles_api_failure_returns_zero() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "agent-1");
        assert_eq!(count_blocker_cycles(&api, "task-1").await, 0);
    }

    #[tokio::test]
    async fn check_next_step_cancelled_returns_cancelled() {
        let server = MockServer::start().await;
        let task_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        Mock::given(method("GET"))
            .and(path_regex(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "cancelled", "playbook_id": null, "playbook_step": 0,
            })))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        assert!(matches!(
            check_next_step(&api, task_id).await.unwrap(),
            StepOutcome::Cancelled { .. }
        ));
    }

    #[tokio::test]
    async fn check_next_step_human_review_returns_already_ready() {
        let server = MockServer::start().await;
        let task_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        Mock::given(method("GET"))
            .and(path_regex(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "human_review", "playbook_id": null, "playbook_step": 0,
            })))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        assert_eq!(
            check_next_step(&api, task_id).await.unwrap(),
            StepOutcome::AlreadyReady
        );
    }

    #[tokio::test]
    async fn check_next_step_backlog_returns_already_ready() {
        let server = MockServer::start().await;
        let task_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        Mock::given(method("GET"))
            .and(path_regex(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "backlog", "playbook_id": null, "playbook_step": 0,
            })))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        assert_eq!(
            check_next_step(&api, task_id).await.unwrap(),
            StepOutcome::AlreadyReady
        );
    }

    #[tokio::test]
    async fn check_next_step_ready_returns_continue() {
        let server = MockServer::start().await;
        let task_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        Mock::given(method("GET"))
            .and(path_regex(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "ready",
            })))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        assert_eq!(
            check_next_step(&api, task_id).await.unwrap(),
            StepOutcome::Continue
        );
    }

    #[tokio::test]
    async fn check_next_step_done_returns_all_done() {
        let server = MockServer::start().await;
        let task_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        Mock::given(method("GET"))
            .and(path_regex(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "done", "playbook_id": null, "playbook_step": 0,
            })))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        assert!(matches!(
            check_next_step(&api, task_id).await.unwrap(),
            StepOutcome::AllDone { .. }
        ));
    }

    #[tokio::test]
    async fn check_next_step_unexpected_state_returns_unexpected_state() {
        let server = MockServer::start().await;
        let task_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        Mock::given(method("GET"))
            .and(path_regex(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "implement",
            })))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        assert_eq!(
            check_next_step(&api, task_id).await.unwrap(),
            StepOutcome::UnexpectedState("implement".to_string())
        );
    }

    #[tokio::test]
    async fn retry_api_call_attempts_4_times_on_persistent_failure() {
        let server = MockServer::start().await;

        let mock = Mock::given(method("GET"))
            .and(path("/tasks/task-retry"))
            .respond_with(ResponseTemplate::new(500))
            .expect(4)
            .mount_as_scoped(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let tid = TaskId::new("task-retry");
        let result = retry_api_call("get_task", &tid, || api.get_task("task-retry")).await;
        assert!(result.is_err());
        drop(mock);
    }

    #[tokio::test]
    async fn retry_api_call_succeeds_on_first_attempt() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-ok"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": "task-ok", "state": "ready"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let tid = TaskId::new("task-ok");
        let result = retry_api_call("get_task", &tid, || api.get_task("task-ok")).await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["id"].as_str(), Some("task-ok"));
    }

    #[tokio::test]
    async fn retry_api_call_succeeds_after_transient_failures() {
        let server = MockServer::start().await;

        // First 2 requests return 500, then succeed
        let fail_mock = Mock::given(method("GET"))
            .and(path("/tasks/task-transient"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .expect(2)
            .mount_as_scoped(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-transient"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": "task-transient", "state": "done"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let tid = TaskId::new("task-transient");
        let result = retry_api_call("get_task", &tid, || api.get_task("task-transient")).await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["state"].as_str(), Some("done"));
        drop(fail_mock);
    }

    #[tokio::test]
    async fn retry_api_call_returns_last_error_on_exhaustion() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-exhaust"))
            .respond_with(ResponseTemplate::new(503))
            .expect(4)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let tid = TaskId::new("task-exhaust");
        let result = retry_api_call("get_task", &tid, || api.get_task("task-exhaust")).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        // Error should mention the HTTP status (503)
        assert!(
            err_msg.contains("503"),
            "Expected error to contain '503', got: {err_msg}"
        );
    }

    // ── resolve_step tests with step_template_id ──

    #[tokio::test]
    async fn resolve_step_uses_resolved_steps_when_present() {
        let server = MockServer::start().await;

        // Playbook response includes resolved_steps (API expanded templates)
        let playbook_with_resolved = serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "impl-stub", "step_template_id": "tmpl-1", "step": 0},
                {"name": "review", "step": 1}
            ],
            "resolved_steps": [
                {"name": "implement", "model": "opus", "budget": 12.0, "step_template_id": "tmpl-1", "step": 0},
                {"name": "review", "step": 1}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(playbook_with_resolved))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        let task = serde_json::json!({
            "id": "task-1",
            "playbook_id": "pb-1",
            "playbook_step": 0
        });

        let (name, step_json) = resolve_step(&api, Some(&task)).await;
        assert_eq!(name, "implement");
        let step = step_json.unwrap();
        assert_eq!(step["model"].as_str(), Some("opus"));
        assert_eq!(step["budget"].as_f64(), Some(12.0));
    }

    #[tokio::test]
    async fn resolve_step_falls_back_to_raw_steps_without_resolved() {
        let server = MockServer::start().await;

        // Playbook response has no resolved_steps — only raw steps
        let playbook = serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "implement", "budget": 5.0, "step": 0},
                {"name": "review", "step": 1}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(playbook))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        let task = serde_json::json!({
            "id": "task-1",
            "playbook_id": "pb-1",
            "playbook_step": 0
        });

        let (name, step_json) = resolve_step(&api, Some(&task)).await;
        assert_eq!(name, "implement");
        let step = step_json.unwrap();
        assert_eq!(step["budget"].as_f64(), Some(5.0));
    }

    #[tokio::test]
    async fn resolve_step_client_side_template_resolution() {
        let server = MockServer::start().await;

        // Playbook has step_template_id but no resolved_steps (old API)
        let playbook = serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "my-impl", "step_template_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", "budget": 20.0, "step": 0}
            ]
        });

        // Template response — provides defaults
        let template = serde_json::json!({
            "id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "name": "implement",
            "model": "opus",
            "budget": 12.0,
            "allowed_tools": "full",
            "description": "Default implement step"
        });

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(playbook))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/step-templates/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"))
            .respond_with(ResponseTemplate::new(200).set_body_json(template))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        let task = serde_json::json!({
            "id": "task-1",
            "playbook_id": "pb-1",
            "playbook_step": 0
        });

        let (name, step_json) = resolve_step(&api, Some(&task)).await;
        // Inline "name" overrides template "name"
        assert_eq!(name, "my-impl");
        let step = step_json.unwrap();
        // Inline budget=20 overrides template budget=12
        assert_eq!(step["budget"].as_f64(), Some(20.0));
        // Template model=opus is inherited (not in inline)
        assert_eq!(step["model"].as_str(), Some("opus"));
        // Template allowed_tools inherited
        assert_eq!(step["allowed_tools"].as_str(), Some("full"));
    }

    #[tokio::test]
    async fn resolve_step_template_fetch_fails_falls_back_gracefully() {
        let server = MockServer::start().await;

        // Playbook has step_template_id but no resolved_steps
        let playbook = serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "implement", "step_template_id": "bad-template-id", "budget": 5.0, "step": 0}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(playbook))
            .mount(&server)
            .await;

        // Template fetch returns 404
        Mock::given(method("GET"))
            .and(path("/step-templates/bad-template-id"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        let task = serde_json::json!({
            "id": "task-1",
            "playbook_id": "pb-1",
            "playbook_step": 0
        });

        let (name, step_json) = resolve_step(&api, Some(&task)).await;
        // Falls back to inline properties
        assert_eq!(name, "implement");
        let step = step_json.unwrap();
        assert_eq!(step["budget"].as_f64(), Some(5.0));
    }

    #[tokio::test]
    async fn resolve_step_no_template_id_unchanged() {
        let server = MockServer::start().await;

        let playbook = serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "implement", "budget": 12.0, "model": "opus", "step": 0}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(playbook))
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        let task = serde_json::json!({
            "id": "task-1",
            "playbook_id": "pb-1",
            "playbook_step": 0
        });

        let (name, step_json) = resolve_step(&api, Some(&task)).await;
        assert_eq!(name, "implement");
        let step = step_json.unwrap();
        assert_eq!(step["budget"].as_f64(), Some(12.0));
        assert_eq!(step["model"].as_str(), Some("opus"));
    }

    #[tokio::test]
    async fn check_next_step_uses_resolved_steps_for_regression() {
        let server = MockServer::start().await;

        // Playbook with resolved_steps — step 0 is retriable (implement)
        let playbook = serde_json::json!({
            "id": "pb-1",
            "steps": [
                {"name": "impl-stub", "step_template_id": "tmpl-1", "step": 0},
                {"name": "review", "step": 1}
            ],
            "resolved_steps": [
                {"name": "implement", "step_template_id": "tmpl-1", "step": 0},
                {"name": "review", "step": 1}
            ]
        });

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_json("ready", 1, "pb-1")))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(playbook))
            .mount(&server)
            .await;

        // Expect regression to step 0 (implement)
        Mock::given(method("PUT"))
            .and(path("/tasks/task-1"))
            .and(body_json(serde_json::json!({"playbook_step": 0})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = test_api(&server.uri());
        let result = check_next_step(&api, "task-1").await;
        assert!(result.is_ok());
    }
}
