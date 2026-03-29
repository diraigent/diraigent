//! `TaskSource` implementation for local orchestration mode.
//!
//! State-mutating operations (claim, transition, cost, updates, locks) go to
//! the local SQLite database. Read-through operations (projects, playbooks,
//! knowledge, decisions, observations, provider config) are forwarded to the
//! API via the inner `ProjectsApi`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use tracing::info;

use crate::db::{self, Db};
use crate::engine::context::ContextAssembler;
use crate::engine::task_source::TaskSource;
use crate::git::ChangedFile;
use crate::project::api::ProjectsApi;

/// Orchestra-local task source: owns the task state machine in SQLite,
/// delegates metadata reads to the API.
pub struct OrchestraTaskSource {
    db: Db,
    api: ProjectsApi,
    context: ContextAssembler,
}

impl OrchestraTaskSource {
    pub fn new(db: Db, api: ProjectsApi) -> Self {
        let context = ContextAssembler::new(api.clone());
        Self { db, api, context }
    }

    /// Resolve the playbook step name for a task (from API playbook data).
    async fn resolve_step_name(&self, task: &Value) -> Result<String> {
        let playbook_id = task["playbook_id"].as_str().unwrap_or("");
        if playbook_id.is_empty() {
            return Ok("working".to_string());
        }
        let playbook = self.api.get_playbook(playbook_id).await?;
        let step_index = task["playbook_step"].as_i64().unwrap_or(0) as usize;
        let name = playbook["steps"]
            .as_array()
            .and_then(|steps| steps.get(step_index))
            .and_then(|step| step["name"].as_str())
            .unwrap_or("implement");
        Ok(name.to_string())
    }

    /// Handle pipeline advancement: when a non-final step completes,
    /// advance to next step instead of terminal "done".
    async fn handle_pipeline_transition(
        &self,
        task_id: &str,
        task: &Value,
        target_state: &str,
    ) -> Result<Value> {
        let current_state = task["state"].as_str().unwrap_or("");

        // Pipeline advancement: non-final step → "done" gets redirected
        if target_state == "done"
            && !diraigent_types::state_machine::is_lifecycle_state(current_state)
            && let Some(playbook_id) = task["playbook_id"].as_str().filter(|s| !s.is_empty())
        {
            let playbook = self.api.get_playbook(playbook_id).await?;
            let current = task["playbook_step"].as_i64().unwrap_or(0) as usize;
            let next = current + 1;
            let total = playbook["steps"].as_array().map(|a| a.len()).unwrap_or(0);

            if next < total {
                // More steps — advance pipeline
                db::task_execution::advance_step(&self.db, task_id)?;
                info!(
                    "pipeline: task {} advanced to step {next}",
                    &task_id[..12.min(task_id.len())]
                );
                return db::task_execution::get(&self.db, task_id)?
                    .ok_or_else(|| anyhow::anyhow!("task not found after advance"));
            }
            // Final step — fall through to normal "done" transition
        }

        // Step regression: non-implement step releasing back to ready
        if target_state == "ready"
            && !diraigent_types::state_machine::is_lifecycle_state(current_state)
            && let Some(playbook_id) = task["playbook_id"].as_str().filter(|s| !s.is_empty())
        {
            let playbook = self.api.get_playbook(playbook_id).await?;
            let current_step = task["playbook_step"].as_i64().unwrap_or(0) as usize;

            if let Some(steps) = playbook["steps"].as_array() {
                let current_retriable = steps
                    .get(current_step)
                    .map(diraigent_types::state_machine::is_retriable_step)
                    .unwrap_or(true);

                if !current_retriable {
                    // Find previous retriable step
                    for prev in (0..current_step).rev() {
                        if let Some(prev_step) = steps.get(prev)
                            && diraigent_types::state_machine::is_retriable_step(prev_step)
                        {
                            db::task_execution::regress_step(&self.db, task_id, prev as i32)?;
                            info!(
                                "pipeline: task {} regressed to step {prev}",
                                &task_id[..12.min(task_id.len())]
                            );
                            return db::task_execution::get(&self.db, task_id)?
                                .ok_or_else(|| anyhow::anyhow!("task not found after regress"));
                        }
                    }
                }
            }
        }

        // Normal transition
        db::task_execution::transition(&self.db, task_id, target_state)?;
        db::task_execution::get(&self.db, task_id)?
            .ok_or_else(|| anyhow::anyhow!("task not found after transition"))
    }
}

#[async_trait]
impl TaskSource for OrchestraTaskSource {
    fn agent_id(&self) -> &str {
        self.api.agent_id()
    }
    fn base_url(&self) -> &str {
        self.api.base_url()
    }
    fn api_token(&self) -> &str {
        self.api.api_token()
    }

    // ── Task lifecycle (LOCAL) ──

    async fn get_task(&self, task_id: &str) -> Result<Value> {
        // Try local first, fall back to API
        if let Some(local) = db::task_execution::get(&self.db, task_id)? {
            return Ok(local);
        }
        self.api.get_task(task_id).await
    }

    async fn get_ready_tasks(&self, project_id: &str) -> Result<Vec<Value>> {
        // Local tasks have priority; also check API for newly created tasks
        let mut local = db::task_execution::get_ready(&self.db, project_id)?;
        // Also fetch from API (these are tasks created via web UI, not yet in local db)
        if let Ok(api_tasks) = self.api.get_ready_tasks(project_id).await {
            for t in api_tasks {
                let id = t["id"].as_str().unwrap_or("");
                if !id.is_empty() {
                    // Register in local db if not already there
                    let state = t["state"].as_str().unwrap_or("ready");
                    db::task_execution::insert(&self.db, id, project_id, state)?;
                    // Only add if not already in local list
                    if !local.iter().any(|l| l["id"].as_str() == Some(id)) {
                        local.push(t);
                    }
                }
            }
        }
        Ok(local)
    }

    async fn claim_task(&self, task_id: &str) -> Result<Value> {
        // Ensure task is in local db
        let task = self.get_task(task_id).await?;
        let project_id = task["project_id"].as_str().unwrap_or("");
        let state = task["state"].as_str().unwrap_or("ready");
        db::task_execution::insert(&self.db, task_id, project_id, state)?;

        // Resolve step name
        let step_name = self.resolve_step_name(&task).await?;
        db::task_execution::claim(&self.db, task_id, &step_name, self.agent_id())?;
        info!(
            "local: claimed {} → {step_name}",
            &task_id[..12.min(task_id.len())]
        );

        db::task_execution::get(&self.db, task_id)?
            .ok_or_else(|| anyhow::anyhow!("task not found after claim"))
    }

    async fn transition_task(&self, task_id: &str, state: &str) -> Result<Value> {
        let task = self.get_task(task_id).await?;
        self.handle_pipeline_transition(task_id, &task, state).await
    }

    async fn transition_task_with_step(
        &self,
        task_id: &str,
        state: &str,
        _playbook_step: u64,
    ) -> Result<Value> {
        // In local mode, pipeline step is managed by handle_pipeline_transition
        self.transition_task(task_id, state).await
    }

    async fn update_task(&self, task_id: &str, body: &Value) -> Result<Value> {
        // Forward to API (human-editable fields like title, kind, context)
        self.api.update_task(task_id, body).await
    }

    async fn create_task(&self, project_id: &str, body: &Value) -> Result<Value> {
        // Create in API (source of truth for task creation)
        let task = self.api.create_task(project_id, body).await?;
        // Register locally
        let id = task["id"].as_str().unwrap_or("");
        let state = task["state"].as_str().unwrap_or("ready");
        if !id.is_empty() {
            db::task_execution::insert(&self.db, id, project_id, state)?;
        }
        Ok(task)
    }

    async fn add_dependency(&self, task_id: &str, depends_on: &str) -> Result<Value> {
        self.api.add_dependency(task_id, depends_on).await
    }

    // ── Task updates, comments, cost (LOCAL) ──

    async fn post_task_update(&self, task_id: &str, kind: &str, content: &str) -> Result<Value> {
        let id = db::task_updates::insert(&self.db, task_id, Some(self.agent_id()), kind, content)?;
        Ok(json!({"id": id, "kind": kind, "content": content}))
    }

    async fn get_task_updates(&self, task_id: &str) -> Result<Vec<Value>> {
        db::task_updates::list_for_task(&self.db, task_id)
    }

    async fn get_task_comments(&self, task_id: &str) -> Result<Vec<Value>> {
        // Comments are human-authored, live in the API
        self.api.get_task_comments(task_id).await
    }

    async fn post_comment(&self, task_id: &str, content: &str) -> Result<Value> {
        self.api.post_comment(task_id, content).await
    }

    async fn post_task_cost(
        &self,
        task_id: &str,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Value> {
        db::task_execution::add_cost(&self.db, task_id, input_tokens, output_tokens, cost_usd)?;
        Ok(json!({}))
    }

    async fn post_changed_files(&self, task_id: &str, files: &[ChangedFile]) -> Result<Value> {
        for f in files {
            db::task_updates::insert_changed_file(&self.db, task_id, &f.path, &f.change_type)?;
        }
        Ok(json!({}))
    }

    // ── Project metadata (API read-through) ──

    async fn get_project(&self, project_id: &str) -> Result<Value> {
        self.api.get_project(project_id).await
    }

    async fn list_projects(&self) -> Result<Vec<Value>> {
        self.api.list_projects().await
    }

    // ── Context (API read-through) ──

    async fn get_context_for_task(&self, project_id: &str, task_id: &str) -> Result<Value> {
        self.context.assemble(project_id, Some(task_id)).await
    }

    async fn get_verifications(&self, project_id: &str, task_id: &str) -> Result<Vec<Value>> {
        // Check local first, then API
        let local = db::task_logs::list_verifications(&self.db, project_id)?;
        if !local.is_empty() {
            return Ok(local);
        }
        self.api.get_verifications(project_id, task_id).await
    }

    async fn get_related_items(&self, task_id: &str) -> Result<Value> {
        self.api.get_related_items(task_id).await
    }

    // ── Playbooks (API read-through) ──

    async fn get_playbook(&self, playbook_id: &str) -> Result<Value> {
        self.api.get_playbook(playbook_id).await
    }

    async fn list_playbooks(&self) -> Result<Vec<Value>> {
        self.api.list_playbooks().await
    }

    async fn create_playbook(&self, body: &Value) -> Result<Value> {
        self.api.create_playbook(body).await
    }

    async fn update_playbook(&self, playbook_id: &str, body: &Value) -> Result<Value> {
        self.api.update_playbook(playbook_id, body).await
    }

    async fn get_step_template(&self, template_id: &str) -> Result<Value> {
        self.api.get_step_template(template_id).await
    }

    // ── Work items (API read-through) ──

    async fn get_work_items(&self, project_id: &str) -> Result<Vec<Value>> {
        self.api.get_work_items(project_id).await
    }

    async fn get_work_item_progress(&self, work_id: &str) -> Result<Value> {
        self.api.get_work_item_progress(work_id).await
    }

    async fn get_task_work_items(&self, task_id: &str) -> Result<Vec<Value>> {
        self.api.get_task_work_items(task_id).await
    }

    async fn get_work_item(&self, work_id: &str) -> Result<Value> {
        self.api.get_work_item(work_id).await
    }

    // ── Events & observations ──

    async fn post_event(&self, project_id: &str, body: &Value) -> Result<Value> {
        let title = body["title"].as_str().unwrap_or("event");
        let kind = body["kind"].as_str().unwrap_or("custom");
        let severity = body["severity"].as_str().unwrap_or("info");
        db::task_logs::insert_event(&self.db, project_id, kind, title, severity, None, None)?;
        Ok(json!({}))
    }

    async fn post_observation(&self, project_id: &str, body: &Value) -> Result<Value> {
        // Observations are shared knowledge — write to API
        self.api.post_observation(project_id, body).await
    }

    async fn list_observations(
        &self,
        project_id: &str,
        status: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<Value>> {
        self.api.list_observations(project_id, status, limit).await
    }

    async fn update_observation(&self, observation_id: &str, body: &Value) -> Result<Value> {
        self.api.update_observation(observation_id, body).await
    }

    // ── Knowledge (API read-through) ──

    async fn post_knowledge(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.context.invalidate(project_id);
        self.api.post_knowledge(project_id, body).await
    }

    async fn list_knowledge(
        &self,
        project_id: &str,
        tag: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<Value>> {
        self.api.list_knowledge(project_id, tag, limit).await
    }

    async fn update_knowledge(&self, knowledge_id: &str, body: &Value) -> Result<Value> {
        self.api.update_knowledge(knowledge_id, body).await
    }

    // ── Decisions (API read-through) ──

    async fn post_decision(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.context.invalidate(project_id);
        self.api.post_decision(project_id, body).await
    }

    async fn list_decisions(&self, project_id: &str) -> Result<Vec<Value>> {
        self.api.list_decisions(project_id).await
    }

    async fn update_decision(&self, decision_id: &str, body: &Value) -> Result<Value> {
        self.api.update_decision(decision_id, body).await
    }

    // ── File locks (LOCAL) ──

    async fn acquire_file_locks(
        &self,
        project_id: &str,
        task_id: &str,
        paths: &[String],
    ) -> Result<Value> {
        db::task_logs::acquire_locks(&self.db, project_id, task_id, self.agent_id(), paths)?;
        Ok(json!({}))
    }

    async fn release_file_locks(&self, _project_id: &str, task_id: &str) -> Result<Value> {
        db::task_logs::release_locks(&self.db, task_id)?;
        Ok(json!({}))
    }

    // ── Provider config (API read-through) ──

    async fn resolve_provider_config(&self, project_id: &str, provider: &str) -> Result<Value> {
        self.api.resolve_provider_config(project_id, provider).await
    }

    // ── Logs (LOCAL) ──

    async fn upload_task_log(
        &self,
        project_id: &str,
        task_id: &str,
        step_name: &str,
        content: &str,
        _metadata: &Value,
    ) -> Result<Value> {
        db::task_logs::insert_log(&self.db, project_id, task_id, step_name, content)?;
        Ok(json!({}))
    }
}
