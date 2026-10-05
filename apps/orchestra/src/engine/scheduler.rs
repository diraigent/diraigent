//! Reaping finished workers and processing their outcomes (merge, cleanup, etc.).

use std::path::Path;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::config::{ActiveTasks, LockQueue};
use crate::engine::task_source::TaskSource;
use crate::git::strategy::GitStrategy;

use crate::project::api::retry_api_call;
use crate::project::paths as project_paths;
use crate::task_id::TaskId;

/// Collect finished tasks and process them (check task state, merge/cleanup).
/// Returns `true` if any file locks were released (triggers immediate re-poll for queued tasks).
pub async fn reap_finished(
    api: &dyn TaskSource,
    projects_path: &Path,
    active: &ActiveTasks,
    lock_queue: &LockQueue,
) -> bool {
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
        .map(|(task_id, handle)| {
            process_reaped_task(api, projects_path, task_id, handle, lock_queue)
        })
        .collect();
    let results = futures_util::future::join_all(futures).await;
    results.iter().any(|released| *released)
}

/// Process a single reaped task: join the handle, check task state, and merge/cleanup.
/// Returns `true` if file locks were released (so queued tasks can be retried).
async fn process_reaped_task(
    api: &dyn TaskSource,
    projects_path: &Path,
    task_id: String,
    handle: JoinHandle<()>,
    lock_queue: &LockQueue,
) -> bool {
    let tid = TaskId::new(task_id.as_str());
    match handle.await {
        Ok(()) => {
            info!("reaped worker {tid}");
        }
        Err(e) => {
            error!("worker {tid} panicked: {e} — skipping merge");
            let msg = format!(
                "Worker panicked (JoinHandle error): {e}. \
                 Worktree preserved for inspection. \
                 Merge skipped."
            );
            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                warn!("failed to post blocker comment for {tid}: {comment_err}");
            }
            return false;
        }
    }

    // Verify the worker left the task in a safe final state
    let outcome = match check_task_outcome(api, &task_id).await {
        Ok(outcome) => outcome,
        Err(e) => {
            error!(
                "completion check API error for {tid}: {e} — skipping merge to avoid pushing incomplete work"
            );
            let msg = format!(
                "Task completion check failed: {e}. \
                 Merge skipped to avoid pushing incomplete work. \
                 Manual intervention may be needed."
            );
            if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                warn!("failed to post completion-error comment for {tid}: {comment_err}");
            }
            let _ = api.transition_task(&task_id, "human_review").await;
            return false;
        }
    };

    // Track project_id for file lock release on terminal outcomes.
    let release_lock_project_id: Option<String>;

    match outcome {
        TaskOutcome::Retry { project_id } => {
            release_lock_project_id = Some(project_id);
            tracing::debug!("task {tid} released for retry");
        }
        TaskOutcome::Done {
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
                    let _ = api
                        .post_comment(
                            &task_id,
                            &format!("Git delivery blocked: {e}. Worktree preserved for review."),
                        )
                        .await;
                    let _ = api.transition_task(&task_id, "human_review").await;
                    let _ = api.release_file_locks(&project_id, &task_id).await;
                    return true;
                }
            };

            if git_strategy.should_merge() {
                let target = git_strategy
                    .merge_target(wm.default_branch())
                    .unwrap_or_else(|| wm.default_branch());
                // Collect stats before merge (branch is deleted after successful merge)
                let branch_name = TaskId::new(&task_id).branch_name();
                let changed_files = wm.collect_changed_files(&task_id).unwrap_or_default();
                let (insertions, deletions) =
                    wm.diff_insertion_deletion_stats(&task_id).unwrap_or((0, 0));
                match wm.merge_to_branch(&task_id, target) {
                    Ok(_) => {
                        let file_paths: Vec<&str> =
                            changed_files.iter().map(|f| f.path.as_str()).collect();
                        emit_merge_event(
                            api,
                            &project_id,
                            &task_id,
                            &branch_name,
                            target,
                            &file_paths,
                            insertions,
                            deletions,
                        )
                        .await;
                        wm.remove_worktree(&task_id);
                    }
                    Err(e) => {
                        error!(
                            "merge failed for {tid}: {e} — keeping branch for manual resolution"
                        );
                        emit_merge_error_event(
                            api,
                            &project_id,
                            &task_id,
                            &branch_name,
                            target,
                            &format!("{e:#}"),
                        )
                        .await;
                        let msg = format!(
                            "Merge to {target} failed: {e}. \
                             Worktree preserved for manual resolution."
                        );
                        if let Err(comment_err) = api.post_comment(&task_id, &msg).await {
                            warn!("failed to post merge-failure comment for {tid}: {comment_err}");
                        }
                        // Transition to human_review so the failure is visible in the review queue
                        if let Err(tr_err) = api.transition_task(&task_id, "human_review").await {
                            warn!(
                                "failed to transition {tid} to human_review after merge failure: {tr_err}"
                            );
                        } else {
                            info!("task {tid} moved to human_review after merge failure");
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
        TaskOutcome::AwaitingReview { project_id } => {
            release_lock_project_id = Some(project_id);
            tracing::debug!("task {tid} in human_review — no action needed");
        }
        TaskOutcome::Cancelled { project_id } => {
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
        TaskOutcome::UnexpectedState(state) => {
            warn!("task {tid} in unexpected state '{state}' — skipping merge, keeping worktree");
            let msg = format!(
                "Task in unexpected state \'{state}\' after worker completed — \
                 skipping merge. \
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

    // Release execution locks after the worker stops; review retains the worktree.
    let mut locks_released = false;
    if let Some(ref pid) = release_lock_project_id {
        match api.release_file_locks(pid, &task_id).await {
            Ok(_) => {
                locks_released = true;
                // Clear lock-queue entries for this project so queued tasks retry immediately.
                let mut queue = lock_queue.lock().await;
                let unblocked: Vec<String> = queue
                    .iter()
                    .filter(|(_, entry)| entry.project_id == *pid)
                    .map(|(task_id, _)| task_id.clone())
                    .collect();
                if !unblocked.is_empty() {
                    for id in &unblocked {
                        queue.remove(id);
                    }
                    info!(
                        "reap {tid}: unblocked {} queued task(s) in project {pid}",
                        unblocked.len()
                    );
                }
            }
            Err(e) => {
                warn!("reap {tid}: failed to release file locks: {e}");
            }
        }
    }
    locks_released
}

#[derive(Debug, PartialEq)]
enum TaskOutcome {
    Retry {
        project_id: String,
    },
    Done {
        project_id: String,
        git_strategy: GitStrategy,
    },
    Cancelled {
        project_id: String,
    },
    AwaitingReview {
        project_id: String,
    },
    UnexpectedState(String),
}
async fn check_task_outcome(api: &dyn TaskSource, task_id: &str) -> anyhow::Result<TaskOutcome> {
    let tid = TaskId::new(task_id);
    let task = retry_api_call("get_task", &tid, || api.get_task(task_id)).await?;
    let project_id = task["project_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("task missing project_id"))?
        .to_owned();
    match task["state"].as_str().unwrap_or("") {
        "done" => {
            let project = api.get_project(&project_id).await?;
            let mode = project["git_mode"].as_str().unwrap_or("standalone");
            let strategy = crate::git::strategy::resolve_strategy(api, Some(&task), mode).await?;
            Ok(TaskOutcome::Done {
                project_id,
                git_strategy: strategy,
            })
        }
        "cancelled" => Ok(TaskOutcome::Cancelled { project_id }),
        "human_review" | "backlog" => Ok(TaskOutcome::AwaitingReview { project_id }),
        "ready" => Ok(TaskOutcome::Retry { project_id }),
        other => Ok(TaskOutcome::UnexpectedState(other.to_owned())),
    }
}

// ── Git event helpers ──

/// Emit a merge success event with file stats.
#[allow(clippy::too_many_arguments)]
async fn emit_merge_event(
    api: &dyn TaskSource,
    project_id: &str,
    task_id: &str,
    branch: &str,
    target_branch: &str,
    files: &[&str],
    insertions: usize,
    deletions: usize,
) {
    let event = serde_json::json!({
        "kind": "merge",
        "source": "orchestra",
        "title": format!("Merged {branch} → {target_branch}"),
        "severity": "info",
        "related_task_id": task_id,
        "agent_id": api.agent_id(),
        "metadata": {
            "task_id": task_id,
            "branch": branch,
            "target_branch": target_branch,
            "files_changed": files.len(),
            "files": files,
            "insertions": insertions,
            "deletions": deletions,
        }
    });
    if let Err(e) = api.post_event(project_id, &event).await {
        warn!("failed to emit merge event: {e}");
    }
}

/// Emit an error event for a failed merge (conflict).
async fn emit_merge_error_event(
    api: &dyn TaskSource,
    project_id: &str,
    task_id: &str,
    branch: &str,
    target_branch: &str,
    error_message: &str,
) {
    let event = serde_json::json!({
        "kind": "error",
        "source": "orchestra",
        "title": format!("Merge conflict: {branch} → {target_branch}"),
        "severity": "warning",
        "related_task_id": task_id,
        "agent_id": api.agent_id(),
        "metadata": {
            "task_id": task_id,
            "branch": branch,
            "target_branch": target_branch,
            "error_message": error_message,
        }
    });
    if let Err(e) = api.post_event(project_id, &event).await {
        warn!("failed to emit merge error event: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectsApi;
    use crate::config::{ActiveTasks, LockQueue};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn human_review_is_held_without_loading_a_git_policy() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id":"task-1", "project_id":"proj-1", "state":"human_review"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proj-1"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        let api = ProjectsApi::new(&server.uri(), "test-agent");
        assert_eq!(
            check_task_outcome(&api, "task-1").await.unwrap(),
            TaskOutcome::AwaitingReview {
                project_id: "proj-1".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn unavailable_git_policy_prevents_completion_delivery() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id":"task-1", "project_id":"proj-1", "state":"done"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proj-1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let api = ProjectsApi::new(&server.uri(), "test-agent");
        assert!(check_task_outcome(&api, "task-1").await.is_err());
    }

    fn new_lock_queue() -> LockQueue {
        Arc::new(Mutex::new(HashMap::new()))
    }

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
                        "id": "task-1", "state": "done",
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

        let api: Arc<dyn TaskSource> = Arc::new(ProjectsApi::new(&server.uri(), "test-agent"));
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
            indexer_interval: 120,
            orchestration_mode: crate::config::OrchestrationMode::Api,
            data_dir: std::env::temp_dir(),
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
            reap_finished(reap_api.as_ref(), &pp, &reap_active, &new_lock_queue()).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let poll_api = Arc::clone(&api);
        let poll_active = Arc::clone(&active);
        let projects: Vec<serde_json::Value> = vec![];
        let poll_result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            crate::engine::spawner::poll_ready_tasks_with_projects(
                &poll_api,
                &config,
                &poll_active,
                &new_lock_queue(),
                &projects,
            ),
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

        reap_finished(&api, &std::env::temp_dir(), &active, &new_lock_queue()).await;

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

        reap_finished(&api, &std::env::temp_dir(), &active, &new_lock_queue()).await;

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
                "id": "task-1", "state": "done",
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

        reap_finished(&api, tmp.path(), &active, &new_lock_queue()).await;

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
                "id": task_id, "state": "cancelled",
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

        reap_finished(&api, tmp.path(), &active, &new_lock_queue()).await;

        assert!(!wt_dir.exists());
        let tasks = active.lock().await;
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn reap_finished_clears_lock_queue_on_lock_release() {
        let server = MockServer::start().await;
        let project_id = "proj-1";

        // Task that is done → triggers lock release
        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "task-1", "state": "done",
                "project_id": project_id,
            })))
            .mount(&server)
            .await;

        mount_nogit_project(&server, project_id).await;

        // Lock release succeeds
        Mock::given(method("DELETE"))
            .and(path("/proj-1/locks/task-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"released": 1})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = ProjectsApi::new(&server.uri(), "test-agent");
        let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));
        let lock_queue = new_lock_queue();

        // Pre-populate lock queue with a task blocked on the same project
        {
            let mut queue = lock_queue.lock().await;
            queue.insert(
                "blocked-task-1".to_string(),
                crate::config::LockQueueEntry {
                    project_id: project_id.to_string(),
                    queued_at: std::time::Instant::now(),
                },
            );
            // Also add a task from a different project (should NOT be cleared)
            queue.insert(
                "other-project-task".to_string(),
                crate::config::LockQueueEntry {
                    project_id: "proj-2".to_string(),
                    queued_at: std::time::Instant::now(),
                },
            );
        }

        // Insert a finished task
        {
            let mut tasks = active.lock().await;
            tasks.insert("task-1".to_string(), tokio::spawn(async {}));
        }
        tokio::task::yield_now().await;

        let locks_released = reap_finished(&api, &std::env::temp_dir(), &active, &lock_queue).await;

        // Should return true (locks were released)
        assert!(locks_released);

        // Blocked task for proj-1 should be removed from queue
        let queue = lock_queue.lock().await;
        assert!(
            !queue.contains_key("blocked-task-1"),
            "blocked task for same project should be dequeued"
        );
        // Task from different project should remain
        assert!(
            queue.contains_key("other-project-task"),
            "task from different project should remain in queue"
        );
    }
}
