//! Reaping finished workers and processing their outcomes (merge, cleanup, etc.).

use std::path::Path;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::api::ProjectsApi;
use crate::config::ActiveTasks;
use crate::git_strategy::GitAction;
use crate::pipeline::{self, StepOutcome};
use crate::project_paths;
use crate::task_id::TaskId;

/// Collect finished tasks and process them (check pipeline state, merge/cleanup).
pub async fn reap_finished(api: &ProjectsApi, projects_path: &Path, active: &ActiveTasks) {
    // Collect finished tasks under a short-lived lock to avoid blocking poll_ready_tasks.
    let finished: Vec<(String, JoinHandle<()>)> = {
        let mut tasks = active.lock().await;
        let finished_ids: Vec<String> = tasks
            .iter()
            .filter(|(_, handle)| handle.is_finished())
            .map(|(id, _)| id.clone())
            .collect();

        finished_ids
            .into_iter()
            .filter_map(|id| tasks.remove(&id).map(|handle| (id, handle)))
            .collect()
    };
    // Lock is dropped here — poll_ready_tasks can proceed concurrently.

    let futures: Vec<_> = finished
        .into_iter()
        .map(|(task_id, handle)| process_reaped_task(api, projects_path, task_id, handle))
        .collect();
    futures_util::future::join_all(futures).await;
}

/// Process a single reaped task: join the handle, check pipeline state, and merge/cleanup.
async fn process_reaped_task(
    api: &ProjectsApi,
    projects_path: &Path,
    task_id: String,
    handle: JoinHandle<()>,
) {
    let tid = TaskId::new(task_id.as_str());
    match handle.await {
        Ok(()) => {
            info!("reaped worker {tid}");
        }
        Err(e) => {
            error!("worker {tid} panicked: {e} — skipping pipeline advancement and merge");
            let msg = format!(
                "Worker panicked (JoinHandle error): {e}. \
                 Worktree preserved for inspection. \
                 Pipeline advancement and merge skipped."
            );
            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                warn!("failed to post blocker comment for {tid}: {comment_err}");
            }
            return;
        }
    }

    // Check if there's a next pipeline step
    let outcome = match pipeline::check_next_step(api, &task_id).await {
        Ok(outcome) => outcome,
        Err(e) => {
            error!(
                "check_next_step API error for {tid}: {e} — skipping merge to avoid pushing incomplete work"
            );
            let msg = format!(
                "Pipeline advancement failed: {e}. \
                 Merge skipped to avoid pushing incomplete work. \
                 Manual intervention may be needed."
            );
            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                warn!("failed to post pipeline-error comment for {tid}: {comment_err}");
            }
            return;
        }
    };

    // Track project_id for file lock release on terminal outcomes.
    let mut release_lock_project_id: Option<String> = None;

    match outcome {
        StepOutcome::Continue => {
            tracing::debug!("task {tid} pipeline continues");
        }
        StepOutcome::ContinueWithGitAction {
            project_id,
            git_strategy,
            git_action,
        } => {
            let wm = match project_paths::create_project_wm(api, &project_id, projects_path).await {
                Ok(wm) => wm,
                Err(e) => {
                    error!(
                        "reap {tid}: failed to resolve project WM for {project_id}: {e} — skipping git action"
                    );
                    return;
                }
            };

            match git_action {
                GitAction::Merge => {
                    let target = git_strategy
                        .merge_target(wm.default_branch())
                        .unwrap_or_else(|| wm.default_branch());
                    match wm.merge_to_branch(&task_id, target) {
                        Ok(_) => {
                            info!("mid-pipeline merge for {tid} to {target} succeeded");
                            wm.remove_worktree(&task_id);
                        }
                        Err(e) => {
                            error!("mid-pipeline merge failed for {tid}: {e} — keeping branch");
                            let msg = format!(
                                "Mid-pipeline merge to {target} failed: {e}. \
                                 Worktree preserved for manual resolution."
                            );
                            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                                warn!(
                                    "failed to post merge-failure comment for {tid}: {comment_err}"
                                );
                            }
                        }
                    }
                }
                GitAction::Push => {
                    if wm.is_git_enabled() {
                        match wm.push_task_branch(&task_id) {
                            Ok(_) => {
                                info!("mid-pipeline push for {tid} succeeded");
                            }
                            Err(e) => {
                                error!("mid-pipeline push failed for {tid}: {e} — continuing");
                                let msg =
                                    format!("Mid-pipeline push failed: {e}. Pipeline continues.");
                                if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                                    warn!(
                                        "failed to post push-failure comment for {tid}: {comment_err}"
                                    );
                                }
                            }
                        }
                    }
                }
                GitAction::None => {}
            }
        }
        StepOutcome::AllDone {
            project_id,
            git_strategy,
        } => {
            release_lock_project_id = Some(project_id.clone());
            let wm = match project_paths::create_project_wm(api, &project_id, projects_path).await {
                Ok(wm) => wm,
                Err(e) => {
                    error!(
                        "reap {tid}: failed to resolve project WM for {project_id}: {e} — skipping merge"
                    );
                    return;
                }
            };

            if git_strategy.should_merge() {
                let target = git_strategy
                    .merge_target(wm.default_branch())
                    .unwrap_or_else(|| wm.default_branch());
                match wm.merge_to_branch(&task_id, target) {
                    Ok(_) => {
                        wm.remove_worktree(&task_id);
                    }
                    Err(e) => {
                        error!(
                            "merge failed for {tid}: {e} — keeping branch for manual resolution"
                        );
                        let msg = format!(
                            "Merge to {target} failed: {e}. \
                             Worktree preserved for manual resolution."
                        );
                        if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                            warn!("failed to post merge-failure comment for {tid}: {comment_err}");
                        }
                    }
                }
            } else if git_strategy.should_push_branch() {
                if wm.is_git_enabled() {
                    match wm.push_task_branch(&task_id) {
                        Ok(_) => {
                            info!("task {tid} branch pushed (branch_only strategy)");
                        }
                        Err(e) => {
                            error!("push task branch failed for {tid}: {e} — keeping branch");
                            let msg = format!(
                                "Push task branch failed: {e}. \
                                 Branch preserved for manual push."
                            );
                            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                                warn!(
                                    "failed to post push-failure comment for {tid}: {comment_err}"
                                );
                            }
                        }
                    }
                }
            } else {
                wm.remove_worktree(&task_id);
            }
        }
        StepOutcome::AlreadyReady => {
            tracing::debug!("task {tid} in human_review — no action needed");
        }
        StepOutcome::Cancelled { project_id } => {
            release_lock_project_id = Some(project_id.clone());
            info!("task {tid} cancelled — removing worktree (no merge)");
            if let Ok(wm) = project_paths::create_project_wm(api, &project_id, projects_path).await
            {
                wm.remove_worktree(&task_id);
            }
            if let Err(e) = api
                .post_comment(
                    &task_id,
                    "Task cancelled. Worktree cleaned up — no merge performed.",
                )
                .await
            {
                warn!("failed to post cancellation comment for {tid}: {e}");
            }
        }
        StepOutcome::UnexpectedState(state) => {
            warn!("task {tid} in unexpected state '{state}' — skipping merge, keeping worktree");
            let msg = format!(
                "Task in unexpected state \'{state}\' after worker completed — \
                 skipping merge and pipeline advancement. \
                 Worktree preserved for investigation."
            );
            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                warn!("failed to post unexpected-state comment for {tid}: {comment_err}");
            }
            // Fetch task to get project_id for lock release
            release_lock_project_id = api
                .get_task(&task_id)
                .await
                .ok()
                .and_then(|t| t["project_id"].as_str().map(|s| s.to_string()));
        }
    }

    // Release file locks for terminal outcomes (fire-and-forget).
    // Continue/ContinueWithGitAction/AlreadyReady keep locks since the task is still in-pipeline.
    if let Some(pid) = release_lock_project_id
        && let Err(e) = api.release_file_locks(&pid, &task_id).await
    {
        warn!("reap {tid}: failed to release file locks: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ProjectsApi;
    use crate::config::ActiveTasks;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Mount a project mock that returns git_mode="none" so create_project_wm
    /// produces a disabled WM. Task JSON must include `project_id` matching this.
    async fn mount_nogit_project(server: &MockServer, project_id: &str) {
        Mock::given(method("GET"))
            .and(path(format!("/{project_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": project_id,
                "slug": "test-project",
                "git_mode": "none",
                "metadata": {}
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn concurrent_reap_does_not_block_poll() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "id": "task-1", "state": "done", "playbook_id": null, "playbook_step": 0,
                        "project_id": "proj-1"
                    }))
                    .set_delay(std::time::Duration::from_millis(200)),
            )
            .mount(&server)
            .await;

        mount_nogit_project(&server, "proj-1").await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;

        let api = Arc::new(ProjectsApi::new(&server.uri(), "test-agent"));
        let config = crate::config::Config {
            agent_id: "test-agent".to_string(),
            project_id: Some("proj-1".to_string()),
            diraigent_api: server.uri(),
            max_workers: 4,
            projects_path: std::env::temp_dir(),
            poll_interval: 30,
            agent_cli: "agent-cli".to_string(),
            log_dir: std::env::temp_dir().join("logs"),
            lockfile: std::env::temp_dir().join(".orchestra.pid"),
            worker_model: None,
            dek: None,
            max_implement_cycles: 3,
        };
        let pp = config.projects_path.clone();
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        {
            let mut tasks = active.lock().await;
            tasks.insert("task-1".to_string(), tokio::spawn(async {}));
        }
        tokio::task::yield_now().await;

        let reap_api = Arc::clone(&api);
        let reap_active = Arc::clone(&active);
        let reap_handle = tokio::spawn(async move {
            reap_finished(&reap_api, &pp, &reap_active).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let poll_api = Arc::clone(&api);
        let poll_active = Arc::clone(&active);
        let poll_result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            crate::spawner::poll_ready_tasks(&poll_api, &config, &poll_active),
        )
        .await;

        assert!(poll_result.is_ok());
        reap_handle.await.unwrap();
    }

    #[tokio::test]
    async fn reap_finished_panic_posts_comment_no_merge() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(0)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        {
            let mut tasks = active.lock().await;
            tasks.insert(
                "task-1".to_string(),
                tokio::spawn(async { panic!("simulated worker panic") }),
            );
        }
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        reap_finished(&api, &std::env::temp_dir(), &active).await;

        let tasks = active.lock().await;
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn reap_finished_check_next_step_err_no_merge() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        {
            let mut tasks = active.lock().await;
            tasks.insert("task-1".to_string(), tokio::spawn(async {}));
        }
        tokio::task::yield_now().await;

        reap_finished(&api, &std::env::temp_dir(), &active).await;

        let tasks = active.lock().await;
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn reap_finished_all_done_nogit_cleans_worktree() {
        let server = MockServer::start().await;
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "task-1", "state": "done", "playbook_id": null, "playbook_step": 0,
                "project_id": project_id,
            })))
            .mount(&server)
            .await;

        mount_nogit_project(&server, project_id).await;

        let tmp = tempfile::tempdir().unwrap();
        let worktree_path = tmp.path().join("worktrees").join("task-task-1");
        std::fs::create_dir_all(&worktree_path).unwrap();
        assert!(worktree_path.exists());

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));

        {
            let mut tasks = active.lock().await;
            tasks.insert("task-1".to_string(), tokio::spawn(async {}));
        }
        tokio::task::yield_now().await;

        reap_finished(&api, tmp.path(), &active).await;

        assert!(!worktree_path.exists());
    }

    #[tokio::test]
    async fn reap_finished_cancelled_removes_worktree_no_merge() {
        let server = MockServer::start().await;
        let task_id = "task-1";
        let project_id = "proj-1";

        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": task_id, "state": "cancelled", "playbook_id": null, "playbook_step": 0,
                "project_id": project_id,
            })))
            .mount(&server)
            .await;

        mount_nogit_project(&server, project_id).await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");

        let tmp = tempfile::tempdir().unwrap();
        let wt_dir = tmp.path().join("worktrees").join("task-task-1");
        std::fs::create_dir_all(&wt_dir).unwrap();
        assert!(wt_dir.exists());

        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));
        {
            let mut tasks = active.lock().await;
            tasks.insert(task_id.to_string(), tokio::spawn(async {}));
        }
        tokio::task::yield_now().await;

        reap_finished(&api, tmp.path(), &active).await;

        assert!(!wt_dir.exists());
        let tasks = active.lock().await;
        assert!(tasks.is_empty());
    }
}
