//! Local task source — reads tasks from a YAML/JSON file, tracks state in memory.
//!
//! Enables headless mode: `orchestra run work.yaml --project-path ./repo`
//! No API connection required.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::engine::task_source::TaskSource;
use crate::git::ChangedFile;

// ── Input file format ────────────────────────────────────────────

/// Top-level work file (YAML or JSON).
#[derive(Debug, Deserialize)]
pub struct WorkFile {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub success_criteria: Option<String>,
    #[serde(default)]
    pub work_type: Option<String>,
    #[serde(default)]
    pub project_path: Option<PathBuf>,
    #[serde(default)]
    pub playbook: Option<String>,
    #[serde(default)]
    pub tasks: Vec<TaskDef>,
    /// If true and tasks is empty, auto-create a single task from the work item.
    #[serde(default = "default_true")]
    pub auto_task: bool,
    /// If true, set decompose=true on the auto-created task.
    #[serde(default)]
    pub decompose: bool,
}

fn default_true() -> bool {
    true
}

/// A task definition within the work file.
#[derive(Debug, Deserialize)]
pub struct TaskDef {
    pub title: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub context: Option<Value>,
    #[serde(default)]
    pub urgent: Option<bool>,
}

// ── Local state ──────────────────────────────────────────────────

struct TaskState {
    data: Value,
    updates: Vec<Value>,
    comments: Vec<Value>,
}

/// A `TaskSource` backed by a local YAML/JSON file.
///
/// - Tasks are generated from the work file at construction time.
/// - State transitions, updates, and comments are tracked in memory.
/// - API-only methods (events, logs, sync) are no-ops.
pub struct LocalTaskSource {
    agent_id: String,
    project_id: String,
    project_path: PathBuf,
    tasks: Mutex<HashMap<String, TaskState>>,
    ready_task_ids: Vec<String>,
}

impl LocalTaskSource {
    /// Load a work file and generate task(s) from it.
    pub fn from_file(path: &Path, project_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading work file {}", path.display()))?;

        let work: WorkFile = if path.extension().is_some_and(|e| e == "json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        let effective_project_path = work
            .project_path
            .as_deref()
            .unwrap_or(project_path)
            .to_path_buf();

        let project_id = Uuid::now_v7().to_string();
        let agent_id = format!("local-{}", &Uuid::now_v7().to_string()[..8]);

        let mut tasks = HashMap::new();
        let mut ready_ids = Vec::new();

        if work.tasks.is_empty() && work.auto_task {
            // Auto-create a single task from the work item
            let task_id = Uuid::now_v7().to_string();
            let mut context = json!({});
            if let Some(ref desc) = work.description {
                context["spec"] = json!(desc);
            }
            if let Some(ref criteria) = work.success_criteria {
                context["acceptance_criteria"] = json!(criteria);
            }
            if work.decompose {
                context["decompose"] = json!(true);
            }

            let data = json!({
                "id": task_id,
                "project_id": project_id,
                "title": work.title,
                "state": "ready",
                "kind": "feature",
                "number": 1,
                "urgent": false,
                "flagged": false,
                "context": context,
                "playbook_id": null,
                "playbook_step": 0,
                "created_at": chrono::Utc::now().to_rfc3339(),
            });

            ready_ids.push(task_id.clone());
            tasks.insert(
                task_id,
                TaskState {
                    data,
                    updates: vec![],
                    comments: vec![],
                },
            );
        } else {
            for (i, task_def) in work.tasks.iter().enumerate() {
                let task_id = Uuid::now_v7().to_string();
                let context = task_def.context.clone().unwrap_or(json!({}));

                let data = json!({
                    "id": task_id,
                    "project_id": project_id,
                    "title": task_def.title,
                    "state": "ready",
                    "kind": task_def.kind.as_deref().unwrap_or("feature"),
                    "number": i + 1,
                    "urgent": task_def.urgent.unwrap_or(false),
                    "flagged": false,
                    "context": context,
                    "playbook_id": null,
                    "playbook_step": 0,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                });

                ready_ids.push(task_id.clone());
                tasks.insert(
                    task_id,
                    TaskState {
                        data,
                        updates: vec![],
                        comments: vec![],
                    },
                );
            }
        }

        tracing::info!(
            "local: loaded {} task(s) from {}",
            tasks.len(),
            path.display()
        );

        Ok(Self {
            agent_id,
            project_id,
            project_path: effective_project_path,
            tasks: Mutex::new(tasks),
            ready_task_ids: ready_ids,
        })
    }

    /// Return the synthetic project ID.
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Return the project path.
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    /// Return task IDs that were created.
    pub fn task_ids(&self) -> &[String] {
        &self.ready_task_ids
    }
}

#[async_trait::async_trait]
impl TaskSource for LocalTaskSource {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }
    fn base_url(&self) -> &str {
        "local://"
    }
    fn api_token(&self) -> &str {
        ""
    }

    // ── Task lifecycle ──

    async fn get_task(&self, task_id: &str) -> Result<Value> {
        let tasks = self.tasks.lock().unwrap();
        tasks
            .get(task_id)
            .map(|t| t.data.clone())
            .ok_or_else(|| anyhow::anyhow!("task {task_id} not found"))
    }

    async fn get_ready_tasks(&self, _project_id: &str) -> Result<Vec<Value>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks
            .values()
            .filter(|t| t.data["state"].as_str() == Some("ready"))
            .map(|t| t.data.clone())
            .collect())
    }

    async fn claim_task(&self, task_id: &str) -> Result<Value> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(task_id) {
            t.data["state"] = json!("implement");
            Ok(t.data.clone())
        } else {
            bail!("task {task_id} not found")
        }
    }

    async fn transition_task(&self, task_id: &str, state: &str) -> Result<Value> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(task_id) {
            t.data["state"] = json!(state);
            tracing::info!("local: task {} → {state}", &task_id[..12]);
            Ok(t.data.clone())
        } else {
            bail!("task {task_id} not found")
        }
    }

    async fn transition_task_with_step(
        &self,
        task_id: &str,
        state: &str,
        playbook_step: u64,
    ) -> Result<Value> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(task_id) {
            t.data["state"] = json!(state);
            t.data["playbook_step"] = json!(playbook_step);
            Ok(t.data.clone())
        } else {
            bail!("task {task_id} not found")
        }
    }

    async fn update_task(&self, task_id: &str, body: &Value) -> Result<Value> {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(task_id) {
            if let Some(obj) = body.as_object() {
                for (k, v) in obj {
                    t.data[k] = v.clone();
                }
            }
            Ok(t.data.clone())
        } else {
            bail!("task {task_id} not found")
        }
    }

    async fn create_task(&self, _project_id: &str, body: &Value) -> Result<Value> {
        let task_id = Uuid::now_v7().to_string();
        let mut data = body.clone();
        data["id"] = json!(task_id);
        data["state"] = json!("ready");
        if data.get("project_id").is_none() {
            data["project_id"] = json!(self.project_id);
        }
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(
            task_id.clone(),
            TaskState {
                data: data.clone(),
                updates: vec![],
                comments: vec![],
            },
        );
        Ok(data)
    }

    async fn add_dependency(&self, _task_id: &str, _depends_on: &str) -> Result<Value> {
        Ok(json!({}))
    }

    // ── Updates, comments, cost ──

    async fn post_task_update(&self, task_id: &str, kind: &str, content: &str) -> Result<Value> {
        let update = json!({
            "kind": kind,
            "content": content,
            "agent_id": self.agent_id,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(task_id) {
            t.updates.push(update.clone());
        }
        tracing::info!("local: [{kind}] {content}");
        Ok(update)
    }

    async fn get_task_updates(&self, task_id: &str) -> Result<Vec<Value>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks
            .get(task_id)
            .map(|t| t.updates.clone())
            .unwrap_or_default())
    }

    async fn get_task_comments(&self, task_id: &str) -> Result<Vec<Value>> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks
            .get(task_id)
            .map(|t| t.comments.clone())
            .unwrap_or_default())
    }

    async fn post_comment(&self, task_id: &str, content: &str) -> Result<Value> {
        let comment = json!({
            "content": content,
            "agent_id": self.agent_id,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(t) = tasks.get_mut(task_id) {
            t.comments.push(comment.clone());
        }
        Ok(comment)
    }

    async fn post_task_cost(
        &self,
        task_id: &str,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Value> {
        tracing::info!(
            "local: task {} cost: ${cost_usd:.4} ({input_tokens}in/{output_tokens}out)",
            &task_id[..12.min(task_id.len())]
        );
        Ok(json!({}))
    }

    async fn post_changed_files(&self, _task_id: &str, _files: &[ChangedFile]) -> Result<Value> {
        Ok(json!({}))
    }

    // ── Project metadata ──

    async fn get_project(&self, _project_id: &str) -> Result<Value> {
        Ok(json!({
            "id": self.project_id,
            "name": "local-project",
            "slug": "local-project",
            "git_mode": "standalone",
            "default_branch": "main",
            "metadata": {},
            "resolved_path": self.project_path.to_string_lossy(),
        }))
    }

    async fn list_projects(&self) -> Result<Vec<Value>> {
        Ok(vec![self.get_project(&self.project_id).await?])
    }

    // ── Context (return empty — no API-side context) ──

    async fn get_context_for_task(&self, _project_id: &str, _task_id: &str) -> Result<Value> {
        Ok(json!({"knowledge": [], "decisions": [], "observations": [], "tasks": []}))
    }

    async fn get_verifications(&self, _project_id: &str, _task_id: &str) -> Result<Vec<Value>> {
        Ok(vec![])
    }

    async fn get_related_items(&self, _task_id: &str) -> Result<Value> {
        Ok(json!({"knowledge": [], "decisions": [], "observations": []}))
    }

    // ── Playbooks (no-op — headless uses defaults) ──

    async fn get_playbook(&self, _playbook_id: &str) -> Result<Value> {
        bail!("no playbooks in local mode")
    }
    async fn list_playbooks(&self) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn create_playbook(&self, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }
    async fn update_playbook(&self, _playbook_id: &str, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }
    async fn get_step_template(&self, _template_id: &str) -> Result<Value> {
        bail!("no step templates in local mode")
    }

    // ── Work items (no-op) ──

    async fn get_work_items(&self, _project_id: &str) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn get_work_item_progress(&self, _work_id: &str) -> Result<Value> {
        Ok(json!({"total_tasks": 0, "done_tasks": 0}))
    }
    async fn get_task_work_items(&self, _task_id: &str) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn get_work_item(&self, _work_id: &str) -> Result<Value> {
        bail!("no work items in local mode")
    }

    // ── Events & observations (log locally) ──

    async fn post_event(&self, _project_id: &str, body: &Value) -> Result<Value> {
        let title = body["title"].as_str().unwrap_or("event");
        tracing::debug!("local event: {title}");
        Ok(json!({}))
    }

    async fn post_observation(&self, _project_id: &str, body: &Value) -> Result<Value> {
        let title = body["title"].as_str().unwrap_or("observation");
        let kind = body["kind"].as_str().unwrap_or("insight");
        tracing::info!("local observation [{kind}]: {title}");
        Ok(json!({}))
    }

    async fn list_observations(
        &self,
        _project_id: &str,
        _status: Option<&str>,
        _limit: Option<i64>,
    ) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn update_observation(&self, _id: &str, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }

    // ── Knowledge (no-op) ──

    async fn post_knowledge(&self, _project_id: &str, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }
    async fn list_knowledge(
        &self,
        _project_id: &str,
        _tag: Option<&str>,
        _limit: Option<i64>,
    ) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn update_knowledge(&self, _id: &str, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }

    // ── Decisions (no-op) ──

    async fn post_decision(&self, _project_id: &str, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }
    async fn list_decisions(&self, _project_id: &str) -> Result<Vec<Value>> {
        Ok(vec![])
    }
    async fn update_decision(&self, _id: &str, _body: &Value) -> Result<Value> {
        Ok(json!({}))
    }

    // ── File locks (no-op — single agent in local mode) ──

    async fn acquire_file_locks(
        &self,
        _project_id: &str,
        _task_id: &str,
        _paths: &[String],
    ) -> Result<Value> {
        Ok(json!({}))
    }
    async fn release_file_locks(&self, _project_id: &str, _task_id: &str) -> Result<Value> {
        Ok(json!({}))
    }

    // ── Provider resolution (return empty — use env vars / defaults) ──

    async fn resolve_provider_config(&self, _project_id: &str, _provider: &str) -> Result<Value> {
        Ok(json!({}))
    }

    // ── Logs (write to stdout) ──

    async fn upload_task_log(
        &self,
        _project_id: &str,
        task_id: &str,
        step_name: &str,
        _content: &str,
        _metadata: &Value,
    ) -> Result<Value> {
        tracing::debug!(
            "local: log for task {} step {step_name} (skipped upload)",
            &task_id[..12.min(task_id.len())]
        );
        Ok(json!({}))
    }
}
