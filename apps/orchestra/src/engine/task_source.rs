//! Trait abstraction over the task lifecycle and project metadata.
//!
//! The engine layer uses `TaskSource` instead of calling `ProjectsApi` directly.
//! This enables headless mode where tasks come from local files instead of the API.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::git::ChangedFile;

/// Core abstraction for task lifecycle, project metadata, and reporting.
///
/// Covers everything the engine layer (spawner, worker, scheduler, pipeline,
/// prompt) needs. The API client implements this directly; a future local
/// implementation can provide tasks from files/stdin.
#[async_trait]
pub trait TaskSource: Send + Sync {
    // ── Identity ──

    fn agent_id(&self) -> &str;
    fn base_url(&self) -> &str;
    fn api_token(&self) -> &str;

    // ── Task lifecycle ──

    async fn get_task(&self, task_id: &str) -> Result<Value>;
    async fn get_ready_tasks(&self, project_id: &str) -> Result<Vec<Value>>;
    async fn claim_task(&self, task_id: &str) -> Result<Value>;
    async fn transition_task(&self, task_id: &str, state: &str) -> Result<Value>;
    async fn transition_task_with_step(
        &self,
        task_id: &str,
        state: &str,
        playbook_step: u64,
    ) -> Result<Value>;
    async fn update_task(&self, task_id: &str, body: &Value) -> Result<Value>;
    async fn create_task(&self, project_id: &str, body: &Value) -> Result<Value>;
    async fn add_dependency(&self, task_id: &str, depends_on: &str) -> Result<Value>;

    // ── Task updates, comments, cost ──

    async fn post_task_update(&self, task_id: &str, kind: &str, content: &str) -> Result<Value>;
    async fn get_task_updates(&self, task_id: &str) -> Result<Vec<Value>>;
    async fn get_task_comments(&self, task_id: &str) -> Result<Vec<Value>>;
    async fn post_comment(&self, task_id: &str, content: &str) -> Result<Value>;
    async fn post_task_cost(
        &self,
        task_id: &str,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Value>;
    async fn post_changed_files(&self, task_id: &str, files: &[ChangedFile]) -> Result<Value>;

    // ── Project metadata ──

    async fn get_project(&self, project_id: &str) -> Result<Value>;
    async fn list_projects(&self) -> Result<Vec<Value>>;

    // ── Context & enrichment ──

    async fn get_context_for_task(&self, project_id: &str, task_id: &str) -> Result<Value>;
    async fn get_verifications(&self, project_id: &str, task_id: &str) -> Result<Vec<Value>>;
    async fn get_related_items(&self, task_id: &str) -> Result<Value>;

    // ── Playbooks ──

    async fn get_playbook(&self, playbook_id: &str) -> Result<Value>;
    async fn list_playbooks(&self) -> Result<Vec<Value>>;
    async fn create_playbook(&self, body: &Value) -> Result<Value>;
    async fn update_playbook(&self, playbook_id: &str, body: &Value) -> Result<Value>;
    async fn get_step_template(&self, template_id: &str) -> Result<Value>;

    // ── Work items ──

    async fn get_work_items(&self, project_id: &str) -> Result<Vec<Value>>;
    async fn get_work_item_progress(&self, work_id: &str) -> Result<Value>;
    async fn get_task_work_items(&self, task_id: &str) -> Result<Vec<Value>>;
    async fn get_work_item(&self, work_id: &str) -> Result<Value>;

    // ── Events & observations ──

    async fn post_event(&self, project_id: &str, body: &Value) -> Result<Value>;
    async fn post_observation(&self, project_id: &str, body: &Value) -> Result<Value>;
    async fn list_observations(
        &self,
        project_id: &str,
        status: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<Value>>;
    async fn update_observation(&self, observation_id: &str, body: &Value) -> Result<Value>;

    // ── Knowledge ──

    async fn post_knowledge(&self, project_id: &str, body: &Value) -> Result<Value>;
    async fn list_knowledge(
        &self,
        project_id: &str,
        tag: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<Value>>;
    async fn update_knowledge(&self, knowledge_id: &str, body: &Value) -> Result<Value>;

    // ── Decisions ──

    async fn post_decision(&self, project_id: &str, body: &Value) -> Result<Value>;
    async fn list_decisions(&self, project_id: &str) -> Result<Vec<Value>>;
    async fn update_decision(&self, decision_id: &str, body: &Value) -> Result<Value>;

    // ── File locks ──

    async fn acquire_file_locks(
        &self,
        project_id: &str,
        task_id: &str,
        paths: &[String],
    ) -> Result<Value>;
    async fn release_file_locks(&self, project_id: &str, task_id: &str) -> Result<Value>;

    // ── Provider resolution ──

    async fn resolve_provider_config(&self, project_id: &str, provider: &str) -> Result<Value>;

    // ── Logs ──

    async fn upload_task_log(
        &self,
        project_id: &str,
        task_id: &str,
        step_name: &str,
        content: &str,
        metadata: &Value,
    ) -> Result<Value>;
}
