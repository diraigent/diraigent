//! Task claiming, loop detection, and worker spawning.

use tracing::{error, info, warn};

use crate::api::ProjectsApi;
use crate::config::{ActiveTasks, Config};
use crate::git::WorktreeManager;
use crate::git_strategy;
use crate::pipeline;
use crate::project_paths;
use crate::step_profile;
use crate::task_id::TaskId;
use crate::worker;

pub async fn poll_ready_tasks(api: &ProjectsApi, config: &Config, active: &ActiveTasks) {
    let tasks = active.lock().await;
    if tasks.len() >= config.max_workers {
        return;
    }
    drop(tasks);

    let projects = match api.list_projects().await {
        Ok(p) => p,
        Err(e) => {
            warn!("poll: failed to fetch projects: {e}");
            return;
        }
    };

    if projects.is_empty() {
        info!("poll: no projects visible to this agent");
        return;
    }
    info!(
        "poll: scanning {} project(s) for ready tasks",
        projects.len()
    );

    for project in &projects {
        let tasks = active.lock().await;
        if tasks.len() >= config.max_workers {
            break;
        }
        drop(tasks);

        let project_id = match project["id"].as_str() {
            Some(id) => id,
            None => continue,
        };

        let ready = match api.get_ready_tasks(project_id).await {
            Ok(t) => t,
            Err(e) => {
                let pname = project["name"]
                    .as_str()
                    .or(project["slug"].as_str())
                    .unwrap_or(project_id);
                warn!("poll: failed to fetch tasks for {pname}: {e}");
                continue;
            }
        };

        if !ready.is_empty() {
            let pname = project["name"]
                .as_str()
                .or(project["slug"].as_str())
                .unwrap_or(project_id);
            info!("poll: {} ready task(s) in {pname}", ready.len());
        }

        for task in &ready {
            let tasks = active.lock().await;
            if tasks.len() >= config.max_workers {
                break;
            }
            let task_id = match task["id"].as_str() {
                Some(id) => id,
                None => continue,
            };
            if tasks.contains_key(task_id) {
                drop(tasks);
                continue;
            }
            drop(tasks);

            let title = task["title"].as_str().unwrap_or("");
            let tid = TaskId::new(task_id);
            info!("poll: picked up {tid} \"{title}\"");

            spawn_worker(api, config, active, task_id, project_id).await;
        }
    }
}

pub async fn spawn_worker(
    api: &ProjectsApi,
    config: &Config,
    active: &ActiveTasks,
    task_id: &str,
    project_id: &str,
) {
    let tid = TaskId::new(task_id);

    // Fetch task for title, per-task model override, and step resolution
    let task_data = api.get_task(task_id).await.ok();
    let title = task_data
        .as_ref()
        .and_then(|t| t["title"].as_str().map(|s| s.to_string()))
        .unwrap_or_default();

    // Resolve playbook step (name + full JSONB config)
    let (step_name, step_json) = pipeline::resolve_step(api, task_data.as_ref()).await;

    // Resolve effective max_cycles: step JSON > project metadata > global config.
    let project_max_cycles =
        pipeline::resolve_max_implement_cycles(api, project_id, config.max_implement_cycles).await;
    let effective_max_cycles = step_json
        .as_ref()
        .and_then(|s| s["max_cycles"].as_u64())
        .map(|v| v as u32)
        .unwrap_or(project_max_cycles);

    // Loop detection: cancel tasks that have failed too many times.
    let step_retriable = step_json
        .as_ref()
        .map(step_profile::is_retriable)
        .unwrap_or_else(|| {
            step_profile::StepProfile::for_step(&step_name) == step_profile::StepProfile::Implement
        });
    if effective_max_cycles > 0 && step_retriable {
        let cycles = pipeline::count_blocker_cycles(api, task_id).await;
        if cycles >= effective_max_cycles {
            warn!(
                "loop-detect {tid}: {cycles} failed implement cycles (max={effective_max_cycles}), cancelling",
            );
            let reason = format!(
                "Needs human review: {cycles} failed implement cycles (threshold: {effective_max_cycles}).",
            );
            if let Err(e) = api.post_task_update(task_id, "blocker", &reason).await {
                warn!("loop-detect {tid}: failed to post blocker: {e}");
            }
            if let Err(e) = api.transition_task(task_id, "ready").await {
                warn!("loop-detect {tid}: failed to release task: {e}");
            } else {
                worker::post_worker_event(
                    api,
                    project_id,
                    task_id,
                    &format!("Loop detected: task {tid} released for human review"),
                    "warn",
                    serde_json::json!({
                        "failed_cycles": cycles,
                        "max_implement_cycles": effective_max_cycles,
                        "step": step_name,
                    }),
                )
                .await;
            }
            return;
        }
    }

    // Per-task model override from task context
    let task_model = task_data
        .as_ref()
        .and_then(|t| t["context"]["model"].as_str().map(|s| s.to_string()));

    // Build step-specific config from playbook JSONB with hardcoded fallbacks
    let step_config = worker::StepConfig::for_step(
        &step_name,
        step_json.as_ref(),
        task_model.as_deref(),
        config.worker_model.as_deref(),
    );

    // Resolve per-project paths from the API.
    let (git_mode, git_root, working_dir, auto_push, default_branch, upload_logs) =
        match project_paths::resolve_project_paths(api, project_id, &config.projects_path).await {
            Ok(paths) => (
                paths.git_mode,
                paths.git_root,
                paths.working_dir,
                paths.auto_push,
                paths.default_branch,
                paths.upload_logs,
            ),
            Err(e) => {
                warn!("spawn {tid}: failed to resolve project paths: {e}");
                (
                    "standalone".to_string(),
                    Some(config.projects_path.clone()),
                    config.projects_path.clone(),
                    false,
                    "main".to_string(),
                    false,
                )
            }
        };

    // Resolve git strategy from playbook metadata
    let git_strategy = git_strategy::resolve_strategy(api, task_data.as_ref(), &git_mode).await;

    let model_info = step_config.model.as_deref().unwrap_or("default");
    info!(
        "spawn {tid}: step={step_name} model={model_info} budget={} tools={} git={}",
        step_config
            .budget
            .map(|b| format!("${b:.1}"))
            .unwrap_or_else(|| "∞".into()),
        step_config.allowed_tools.len(),
        git_strategy.id(),
    );

    // Auto-clone/init repo if it doesn't exist yet
    if git_mode != "none"
        && let Some(ref root) = git_root
        && !root.join(".git").exists()
    {
        info!(
            "spawn {tid}: repo not found at {}, provisioning...",
            root.display()
        );
        if let Ok(project) = api.get_project(project_id).await {
            let repo_url = project["repo_url"].as_str().unwrap_or("");
            let default_branch = project["default_branch"].as_str().unwrap_or("main");
            let slug = project["slug"].as_str().unwrap_or(project_id);
            crate::git_provisioner::provision_repo(root, repo_url, default_branch, slug);
        } else {
            warn!("spawn {tid}: could not fetch project record for git provisioning");
        }
    }

    let api_clone = api.clone();
    let agent_cli = config.agent_cli.clone();
    let log_dir = config.log_dir.clone();
    let task_id_owned = task_id.to_string();
    let project_id_owned = project_id.to_string();
    let dek = config.dek.clone();

    let handle = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            // Use strategy's base_branch (falls back to default_branch for Merge)
            let base = git_strategy
                .base_branch(&default_branch)
                .unwrap_or(&default_branch);
            let wm = if git_mode == "none" || git_strategy == crate::git_strategy::GitStrategy::NoGit {
                WorktreeManager::disabled(&working_dir)
            } else if let Some(ref root) = git_root {
                let m = WorktreeManager::with_branch(root, base);
                // For feature_branch, ensure the goal branch exists before creating worktrees
                if let crate::git_strategy::GitStrategy::FeatureBranch { goal_branch } =
                    &git_strategy
                    && let Err(e) = m.ensure_branch(goal_branch)
                {
                    warn!(
                        "spawn: failed to ensure goal branch {goal_branch} for task {task_id_owned}: {e}"
                    );
                }
                m
            } else {
                WorktreeManager::disabled(&working_dir)
            };
            wm.set_auto_push(auto_push);
            let repo_root_for_worker = git_root.as_deref().unwrap_or(&working_dir);
            match worker::run_worker(
                &api_clone,
                &wm,
                &task_id_owned,
                &project_id_owned,
                repo_root_for_worker,
                &agent_cli,
                &log_dir,
                &step_config,
                dek.as_ref(),
                upload_logs,
            )
            .await
            {
                Ok(result) => {
                    let sid = TaskId::new(task_id_owned.as_str());
                    if result.is_error {
                        error!(
                            "worker {sid} completed with error: stop_reason={} cost=${:.2} turns={} duration={}s",
                            result.stop_reason, result.cost_usd, result.api_turns, result.duration_seconds
                        );
                    }
                }
                Err(e) => {
                    let sid = TaskId::new(task_id_owned.as_str());
                    error!("worker {sid} crashed: {e:#}");

                    // Post blocker so humans can see why the task failed
                    let blocker_msg = format!("Worker crashed: {e:#}");
                    if let Err(be) = api_clone.post_task_update(&task_id_owned, "blocker", &blocker_msg).await {
                        warn!("worker {sid}: failed to post blocker: {be}");
                    }

                    // Transition task to cancelled to prevent infinite crash loop.
                    // Infrastructure errors (worktree creation, git issues) are not
                    // transient — re-picking the task will just crash again.
                    if let Err(te) = api_clone.transition_task(&task_id_owned, "cancelled").await {
                        warn!("worker {sid}: failed to cancel crashed task: {te}");
                    } else {
                        info!("worker {sid}: cancelled crashed task to prevent retry loop");
                    }

                    worker::post_worker_event(
                        &api_clone,
                        &project_id_owned,
                        &task_id_owned,
                        &format!("Worker crashed: task {sid}"),
                        "error",
                        serde_json::json!({ "error": format!("{e:#}") }),
                    )
                    .await;
                }
            }
        });
    });

    let count = {
        let mut tasks = active.lock().await;
        tasks.insert(task_id.to_string(), handle);
        tasks.len()
    };

    info!("start {tid} ({count}/{}) \"{title}\"", config.max_workers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ProjectsApi;
    use crate::config::{ActiveTasks, Config};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use wiremock::matchers::{method, path};
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

    fn project_json(metadata: Option<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({
            "id": "proj-1",
            "name": "test-project",
            "slug": "test-project",
            "git_mode": "none",
            "metadata": metadata.unwrap_or(serde_json::json!({}))
        })
    }

    async fn mount_project_mock(server: &MockServer, metadata: Option<serde_json::Value>) {
        Mock::given(method("GET"))
            .and(path("/proj-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(project_json(metadata)))
            .mount(server)
            .await;
    }

    fn test_config(base_url: &str, max_implement_cycles: u32) -> Config {
        Config {
            agent_id: "test-agent".to_string(),
            project_id: Some("proj-1".to_string()),
            diraigent_api: base_url.to_string(),
            max_workers: 4,
            projects_path: std::env::temp_dir(),
            poll_interval: 30,
            agent_cli: "agent-cli".to_string(),
            log_dir: std::env::temp_dir().join("logs"),
            lockfile: std::env::temp_dir().join(".orchestra.pid"),
            worker_model: None,
            dek: None,
            max_implement_cycles,
        }
    }

    #[tokio::test]
    async fn spawn_worker_cancels_at_loop_detect_threshold() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "title": "Test task", "state": "implement",
                "playbook_id": "pb-1", "playbook_step": 0, "context": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"kind": "blocker", "content": "build failed", "agent_id": "test-agent"},
                {"kind": "blocker", "content": "test failed", "agent_id": "test-agent"},
                {"kind": "blocker", "content": "lint failed", "agent_id": "test-agent"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/transition"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/proj-1/events"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        mount_project_mock(&server, None).await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let config = test_config(&server.uri(), 3);
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        spawn_worker(&api, &config, &active, task_id, project_id).await;

        let tasks = active.lock().await;
        assert!(!tasks.contains_key(task_id));
    }

    #[tokio::test]
    async fn spawn_worker_below_threshold_proceeds_normally() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "title": "Test task", "state": "implement",
                "playbook_id": "pb-1", "playbook_step": 0, "context": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"kind": "blocker", "content": "build failed", "agent_id": "test-agent"},
                {"kind": "blocker", "content": "test failed", "agent_id": "test-agent"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/transition"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/proj-1/events"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        mount_project_mock(&server, None).await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let config = test_config(&server.uri(), 3);
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        spawn_worker(&api, &config, &active, task_id, project_id).await;
    }

    #[tokio::test]
    async fn spawn_worker_loop_detect_disabled_when_max_cycles_zero() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "title": "Test task", "state": "implement",
                "playbook_id": "pb-1", "playbook_step": 0, "context": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(0)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/transition"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        mount_project_mock(&server, None).await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let config = test_config(&server.uri(), 0);
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        spawn_worker(&api, &config, &active, task_id, project_id).await;
    }

    #[tokio::test]
    async fn spawn_worker_review_step_skips_loop_detection() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "title": "Test task", "state": "review",
                "playbook_id": "pb-1", "playbook_step": 1, "context": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(0)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/transition"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        mount_project_mock(&server, None).await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let config = test_config(&server.uri(), 3);
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        spawn_worker(&api, &config, &active, task_id, project_id).await;
    }

    #[tokio::test]
    async fn spawn_worker_project_override_max_cycles_1_triggers_cancellation() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "title": "Test task", "state": "implement",
                "playbook_id": "pb-1", "playbook_step": 0, "context": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        mount_project_mock(
            &server,
            Some(serde_json::json!({"max_implement_cycles": 1})),
        )
        .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"kind": "blocker", "content": "build failed", "agent_id": "test-agent"}
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/transition"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/proj-1/events"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let config = test_config(&server.uri(), 3);
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        spawn_worker(&api, &config, &active, task_id, project_id).await;

        let tasks = active.lock().await;
        assert!(!tasks.contains_key(task_id));
    }

    #[tokio::test]
    async fn spawn_worker_project_override_max_cycles_0_disables_loop_detection() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "title": "Test task", "state": "implement",
                "playbook_id": "pb-1", "playbook_step": 0, "context": {}
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/playbooks/pb-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(standard_playbook()))
            .mount(&server)
            .await;

        mount_project_mock(
            &server,
            Some(serde_json::json!({"max_implement_cycles": 0})),
        )
        .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1/updates"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(0)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/transition"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/proj-1/events"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let config = test_config(&server.uri(), 3);
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        spawn_worker(&api, &config, &active, task_id, project_id).await;
    }
}
