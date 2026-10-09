//! `TaskSource` implementation for local orchestration mode.
//!
//! State-mutating operations (claim, transition, cost, updates, locks) go to
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
}

#[async_trait]
impl TaskSource for OrchestraTaskSource {
    async fn resolve_mcp_sessions(
        &self,
        project: &str,
        task: &str,
        profile: crate::engine::task_profile::TaskProfile,
        provider: &str,
        selection: Option<crate::engine::mcp::Selection>,
    ) -> Result<crate::engine::mcp::Sessions> {
        crate::engine::mcp::resolve(&self.api, project, task, profile, provider, selection).await
    }
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
            // SQLite owns execution state; the API owns the spec and worker options.
            let mut task = self
                .api
                .get_task(task_id)
                .await
                .unwrap_or_else(|_| json!({}));
            if let (Some(task), Some(local)) = (task.as_object_mut(), local.as_object()) {
                task.extend(local.clone());
            }
            return Ok(task);
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
                    if !local.iter().any(|l| l["id"].as_str() == Some(id))
                        && db::task_execution::get(&self.db, id)?
                            .is_some_and(|task| task["state"] == "ready")
                    {
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
        let step_name = "working";
        db::task_execution::claim(&self.db, task_id, self.agent_id())?;
        info!(
            "local: claimed {} → {step_name}",
            &task_id[..12.min(task_id.len())]
        );

        db::task_execution::get(&self.db, task_id)?
            .ok_or_else(|| anyhow::anyhow!("task not found after claim"))
    }

    async fn transition_task(&self, task_id: &str, state: &str) -> Result<Value> {
        db::task_execution::transition(&self.db, task_id, state)?;
        db::task_execution::get(&self.db, task_id)?
            .ok_or_else(|| anyhow::anyhow!("task not found after transition"))
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
        if kind == "artifact" {
            let task = self.api.get_task(task_id).await?;
            if self
                .api
                .content_owner(task["project_id"].as_str().unwrap_or(""))
                .await?
                .is_some()
            {
                return self.api.post_task_update(task_id, kind, content).await;
            }
        }
        let id = db::task_updates::insert(&self.db, task_id, Some(self.agent_id()), kind, content)?;
        Ok(json!({"id": id, "kind": kind, "content": content}))
    }

    async fn get_task_updates(&self, task_id: &str) -> Result<Vec<Value>> {
        let task = self.api.get_task(task_id).await?;
        if self
            .api
            .content_owner(task["project_id"].as_str().unwrap_or(""))
            .await?
            .is_some()
        {
            return self.api.get_task_updates(task_id).await;
        }
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
        let task = self.api.get_task(task_id).await?;
        if self
            .api
            .content_owner(task["project_id"].as_str().unwrap_or(""))
            .await?
            .is_some()
        {
            return self.api.post_changed_files(task_id, files).await;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };
    #[tokio::test]
    async fn local_review_hold_keeps_remote_spec_and_worker_options() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let db = db::open(dir.path()).unwrap();
        db::task_execution::insert(&db, "task-1", "proj-1", "human_review").unwrap();
        let remote = json!({"id":"task-1","project_id":"proj-1","state":"ready",
            "context":{"spec":"Review the change","mode":"review","worker":{"provider":"opencode"}}});
        Mock::given(method("GET"))
            .and(path("/tasks/task-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&remote))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/proj-1/tasks/ready"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([remote.clone()])))
            .mount(&server)
            .await;
        let source = OrchestraTaskSource::new(db, ProjectsApi::new(&server.uri(), "agent"));
        let task = source.get_task("task-1").await.unwrap();
        assert_eq!(task["state"], "human_review");
        assert_eq!(task["context"], remote["context"]);
        assert!(source.get_ready_tasks("proj-1").await.unwrap().is_empty());
    }
}
