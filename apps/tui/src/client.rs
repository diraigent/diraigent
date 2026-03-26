use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    pub base_url: String,
    token: String,
}

// API types
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    #[serde(default)]
    pub default_playbook_id: Option<Uuid>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub repo_path: Option<String>,
    #[serde(default)]
    pub service_name: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeMdResponse {
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    pub id: Uuid,
    #[serde(default)]
    pub number: i64,
    pub title: String,
    #[serde(default)]
    pub kind: String,
    pub state: String,
    #[serde(default)]
    pub urgent: bool,
    pub assigned_agent_id: Option<Uuid>,
    #[serde(default)]
    pub context: serde_json::Value,
    pub playbook_id: Option<Uuid>,
    pub playbook_step: Option<i32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub flagged: bool,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskUpdate {
    pub id: Option<Uuid>,
    pub kind: String,
    pub content: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskComment {
    pub id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub content: String,
    pub created_at: Option<String>,
    #[serde(default)]
    pub author_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub status: String,
    pub last_seen_at: Option<String>,
    #[serde(default)]
    pub owner_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Decision {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub alternatives: Option<Vec<DecisionAlternative>>,
    #[serde(default)]
    pub consequences: Option<String>,
    #[serde(default)]
    pub superseded_by: Option<Uuid>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A single alternative considered in a decision.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DecisionAlternative {
    pub name: String,
    #[serde(default)]
    pub pros: Option<String>,
    #[serde(default)]
    pub cons: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Work {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub work_type: Option<String>,
    #[serde(default)]
    pub parent_work_id: Option<Uuid>,
    #[serde(default)]
    pub auto_status: Option<bool>,
    #[serde(default)]
    pub success_criteria: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkProgress {
    pub work_id: Uuid,
    pub total_tasks: i64,
    pub done_tasks: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkStats {
    pub work_id: Uuid,
    pub backlog_count: i64,
    pub ready_count: i64,
    pub working_count: i64,
    pub done_count: i64,
    pub cancelled_count: i64,
    pub total_count: i64,
    pub kind_breakdown: serde_json::Value,
    pub total_cost_usd: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub blocked_count: i64,
    pub avg_completion_hours: Option<f64>,
    pub oldest_open_task_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Observation {
    pub id: Uuid,
    #[serde(default)]
    pub agent_id: Option<Uuid>,
    #[serde(default)]
    pub kind: Option<String>,
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub evidence: Option<serde_json::Value>,
    pub resolved_task_id: Option<Uuid>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_task_id: Option<Uuid>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskDependencyInfo {
    pub task_id: Uuid,
    pub depends_on: Uuid,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskDependencies {
    #[serde(default)]
    pub depends_on: Vec<TaskDependencyInfo>,
    #[serde(default)]
    pub blocks: Vec<TaskDependencyInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Integration {
    pub id: Uuid,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub auth_type: Option<String>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ── Log types ─────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub line: String,
    #[serde(default)]
    pub labels: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogsResponse {
    pub entries: Vec<LogEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LokiLabelsResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub data: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IntegrationAccess {
    pub integration_id: Uuid,
    pub agent_id: Uuid,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditEntry {
    pub id: Uuid,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    pub actor_agent_id: Option<Uuid>,
    #[serde(default)]
    pub actor_user_id: Option<Uuid>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub entity_id: Option<Uuid>,
    #[serde(default)]
    pub summary: Option<String>,
    pub before_state: Option<serde_json::Value>,
    pub after_state: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitTaskStatus {
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub exists: bool,
    #[serde(default)]
    pub ahead: i32,
    #[serde(default)]
    pub behind: i32,
    #[serde(default)]
    pub changed_files_count: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChangedFile {
    pub id: Option<Uuid>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub additions: i32,
    #[serde(default)]
    pub deletions: i32,
}

// ── Git branch types ─────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub commit: String,
    #[serde(default)]
    pub is_pushed: bool,
    #[serde(default)]
    pub ahead_remote: i32,
    #[serde(default)]
    pub behind_remote: i32,
    pub task_id_prefix: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BranchListResponse {
    pub current_branch: String,
    pub branches: Vec<BranchInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MainPushStatus {
    #[serde(default)]
    pub ahead: i32,
    #[serde(default)]
    pub behind: i32,
    pub last_commit: Option<String>,
    pub last_commit_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PushResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: String,
}

// ── Search types ─────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResult {
    pub entity_type: String,
    pub id: Uuid,
    pub title: String,
    pub snippet: Option<String>,
    #[serde(default)]
    pub relevance: f32,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub query: String,
}

// ── Chat types ───────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// ── Source browser types ─────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TreeResponse {
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlobResponse {
    pub content: String,
    #[serde(default)]
    pub encoding: String,
    #[serde(default)]
    pub size: usize,
}

// ── Report types ─────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Report {
    pub id: Uuid,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    #[serde(default)]
    pub created_by: Option<Uuid>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ── Event types ──────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Event {
    pub id: Uuid,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub related_task_id: Option<Uuid>,
    #[serde(default)]
    pub agent_id: Option<Uuid>,
    #[serde(default)]
    pub created_at: Option<String>,
}

// Type alias so views can use ProjectEvent as the canonical name
pub type ProjectEvent = Event;
// ── Webhook types ────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Webhook {
    pub id: Uuid,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    #[serde(default)]
    pub webhook_id: Option<Uuid>,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub response_status: Option<i32>,
    #[serde(default)]
    pub response_body: Option<String>,
    #[serde(default)]
    pub delivered_at: Option<String>,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub attempt_number: i32,
}

// ── Metrics types ────────────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectMetrics {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub range_days: i32,
    #[serde(default)]
    pub task_summary: Option<TaskSummary>,
    #[serde(default)]
    pub tasks_per_day: Vec<DayCount>,
    #[serde(default)]
    pub avg_time_in_state_hours: Vec<StateAvg>,
    #[serde(default)]
    pub agent_breakdown: Vec<AgentMetrics>,
    #[serde(default)]
    pub playbook_completion: Vec<PlaybookMetrics>,
    #[serde(default)]
    pub cost_summary: Option<CostSummary>,
    #[serde(default)]
    pub task_costs: Vec<TaskCostRow>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TaskSummary {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub done: i64,
    #[serde(default)]
    pub cancelled: i64,
    #[serde(default)]
    pub in_progress: i64,
    #[serde(default)]
    pub ready: i64,
    #[serde(default)]
    pub backlog: i64,
    #[serde(default)]
    pub human_review: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CostSummary {
    #[serde(default)]
    pub total_input_tokens: i64,
    #[serde(default)]
    pub total_output_tokens: i64,
    #[serde(default)]
    pub total_cost_usd: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DayCount {
    #[serde(default)]
    pub day: String,
    #[serde(default)]
    pub count: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateAvg {
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub avg_hours: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentMetrics {
    #[serde(default)]
    pub agent_id: Option<Uuid>,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub tasks_completed: i64,
    #[serde(default)]
    pub tasks_in_progress: i64,
    #[serde(default)]
    pub avg_completion_hours: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlaybookMetrics {
    #[serde(default)]
    pub playbook_id: Option<Uuid>,
    #[serde(default)]
    pub playbook_title: String,
    #[serde(default)]
    pub total_tasks: i64,
    #[serde(default)]
    pub completed_tasks: i64,
    #[serde(default)]
    pub completion_rate: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskCostRow {
    #[serde(default)]
    pub task_id: Option<Uuid>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
}

// ── Work Comment types ───────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkComment {
    pub id: Uuid,
    #[serde(default)]
    pub work_id: Option<Uuid>,
    #[serde(default)]
    pub agent_id: Option<Uuid>,
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ── Observation cleanup types ────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CleanupObservationsResult {
    #[serde(default)]
    pub deleted_dismissed: i64,
    #[serde(default)]
    pub deleted_acknowledged: i64,
    #[serde(default)]
    pub deleted_acted_on: i64,
    #[serde(default)]
    pub deleted_resolved: i64,
    #[serde(default)]
    pub deleted_duplicates: i64,
    #[serde(default)]
    pub total_deleted: i64,
}

#[allow(dead_code)]
impl ApiClient {
    pub fn new() -> Self {
        let base_url = std::env::var("DIRAIGENT_API_URL")
            .unwrap_or_else(|_| "http://localhost:8082/v1".to_string());
        let token = std::env::var("DIRAIGENT_API_TOKEN").unwrap_or_else(|_| "dev".to_string());

        Self {
            client: Client::new(),
            base_url,
            token,
        }
    }

    /// Auth header only — no X-Agent-Id since TUI is a human-facing tool.
    /// Without X-Agent-Id, the API treats requests as human/admin (authz skipped).
    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("Authorization", format!("Bearer {}", self.token))
    }

    pub async fn health(&self) -> Result<bool, reqwest::Error> {
        let health_url = self.base_url.replace("/v1", "/health/live");
        let resp = self.client.get(&health_url).send().await?;
        Ok(resp.status().is_success())
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>, reqwest::Error> {
        let req = self.client.get(&self.base_url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn update_project(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Project, reqwest::Error> {
        let req = self.client.put(format!("{}/{}", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn get_claude_md(&self, project_id: Uuid) -> Result<String, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/claude-md", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        let body: ClaudeMdResponse = resp.json().await?;
        Ok(body.content)
    }

    pub async fn update_claude_md(
        &self,
        project_id: Uuid,
        content: &str,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/{}/claude-md", self.base_url, project_id))
            .json(&serde_json::json!({"content": content}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn list_tasks(
        &self,
        project_id: Uuid,
    ) -> Result<PaginatedResponse<Task>, reqwest::Error> {
        let limit = 100;
        let mut all = Vec::new();
        let mut offset = 0u64;
        loop {
            let req = self.client.get(format!(
                "{}/{}/tasks?limit={}&offset={}",
                self.base_url, project_id, limit, offset
            ));
            let page: PaginatedResponse<Task> = self
                .auth(req)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            all.extend(page.data);
            if !page.has_more {
                break;
            }
            offset += limit;
        }
        Ok(PaginatedResponse {
            data: all,
            has_more: false,
        })
    }

    pub async fn get_task_updates(&self, task_id: Uuid) -> Result<Vec<TaskUpdate>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/tasks/{}/updates", self.base_url, task_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn get_task_comments(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<TaskComment>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/tasks/{}/comments", self.base_url, task_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn update_task(
        &self,
        task_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Task, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/tasks/{}", self.base_url, task_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_task(
        &self,
        project_id: Uuid,
        title: &str,
        kind: &str,
        urgent: bool,
        spec: &str,
        work_id: Option<Uuid>,
    ) -> Result<Task, reqwest::Error> {
        let mut body = serde_json::json!({
            "title": title,
            "kind": kind,
            "urgent": urgent,
            "context": { "spec": spec }
        });
        if let Some(wid) = work_id {
            body["work_id"] = serde_json::json!(wid);
        }
        let req = self
            .client
            .post(format!("{}/{}/tasks", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn list_subtasks(&self, task_id: Uuid) -> Result<Vec<Task>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/tasks/{}/subtasks", self.base_url, task_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_agents(&self) -> Result<Vec<Agent>, reqwest::Error> {
        let req = self.client.get(format!("{}/agents", self.base_url));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_decisions(&self, project_id: Uuid) -> Result<Vec<Decision>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/decisions", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Work operations ──────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub async fn create_work(
        &self,
        project_id: Uuid,
        title: &str,
        description: &str,
        work_type: &str,
        parent_work_id: Option<Uuid>,
        auto_status: bool,
    ) -> Result<Work, reqwest::Error> {
        let mut body = serde_json::json!({
            "title": title,
            "description": description,
            "work_type": work_type,
            "auto_status": auto_status,
        });
        if let Some(pid) = parent_work_id {
            body["parent_work_id"] = serde_json::json!(pid);
        }
        let req = self
            .client
            .post(format!("{}/{}/work", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn create_work_comment(
        &self,
        work_id: Uuid,
        body: serde_json::Value,
    ) -> Result<WorkComment, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/work/{}/comments", self.base_url, work_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn list_work_filtered(
        &self,
        project_id: Uuid,
        status_not: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Work>, reqwest::Error> {
        let mut url = format!("{}/{}/work", self.base_url, project_id);
        let mut params = Vec::new();
        if let Some(sn) = status_not {
            params.push(format!("status_not={}", sn));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn get_work_progress(&self, work_id: Uuid) -> Result<WorkProgress, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/work/{}/progress", self.base_url, work_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn get_work_stats(&self, work_id: Uuid) -> Result<WorkStats, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/work/{}/stats", self.base_url, work_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_work_children(&self, work_id: Uuid) -> Result<Vec<Work>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/work/{}/children", self.base_url, work_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_work(
        &self,
        work_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Work, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/work/{}", self.base_url, work_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn create_task_with_json(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Task, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/tasks", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn list_work_tasks(
        &self,
        work_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, reqwest::Error> {
        let req = self.client.get(format!(
            "{}/work/{}/tasks?limit={}&offset={}",
            self.base_url, work_id, limit, offset
        ));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Observation operations ────────────────────────────────

    pub async fn list_observations(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Observation>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/observations", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn update_observation(
        &self,
        id: Uuid,
        body: serde_json::Value,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/observations/{}", self.base_url, id));
        self.auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn create_observation(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Observation, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/observations", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn dismiss_observation(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/observations/{}/dismiss", self.base_url, id))
            .json(&serde_json::json!({}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn promote_observation(
        &self,
        id: Uuid,
        title: &str,
        kind: &str,
        urgent: bool,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/observations/{}/promote", self.base_url, id))
            .json(&serde_json::json!({"title": title, "kind": kind, "urgent": urgent}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Knowledge/Decision create & edit operations ─────────────────────

    pub async fn create_decision(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Decision, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/decisions", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_decision(
        &self,
        id: Uuid,
        body: serde_json::Value,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/decisions/{}", self.base_url, id));
        self.auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn delete_decision(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/decisions/{}", self.base_url, id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Task dependency operations ────────────────────────────

    pub async fn list_task_dependencies(
        &self,
        task_id: Uuid,
    ) -> Result<TaskDependencies, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/tasks/{}/dependencies", self.base_url, task_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn add_dependency(
        &self,
        task_id: Uuid,
        depends_on: Uuid,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/tasks/{}/dependencies", self.base_url, task_id))
            .json(&serde_json::json!({"depends_on": depends_on}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Integration operations ────────────────────────────────

    pub async fn list_integrations(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Integration>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/integrations", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn create_integration(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Integration, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/integrations", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn delete_integration(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/integrations/{}", self.base_url, id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn toggle_integration(&self, id: Uuid, enabled: bool) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/integrations/{}", self.base_url, id))
            .json(&serde_json::json!({"enabled": enabled}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn list_integration_access(
        &self,
        id: Uuid,
    ) -> Result<Vec<IntegrationAccess>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/integrations/{}/access", self.base_url, id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn grant_integration_access(
        &self,
        id: Uuid,
        agent_id: Uuid,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/integrations/{}/access", self.base_url, id))
            .json(&serde_json::json!({"agent_id": agent_id}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn revoke_integration_access(
        &self,
        id: Uuid,
        agent_id: Uuid,
    ) -> Result<(), reqwest::Error> {
        let req = self.client.delete(format!(
            "{}/integrations/{}/access/{}",
            self.base_url, id, agent_id
        ));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Audit operations ──────────────────────────────────────

    pub async fn list_audit(&self, project_id: Uuid) -> Result<Vec<AuditEntry>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/audit?limit=200", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Log operations ──────────────────────────────────────

    pub async fn query_logs(
        &self,
        query: &str,
        start: Option<&str>,
        end: Option<&str>,
        limit: u32,
        direction: &str,
    ) -> Result<LogsResponse, reqwest::Error> {
        let mut url = format!(
            "{}/logs?query={}&limit={}&direction={}",
            self.base_url,
            urlencoding::encode(query),
            limit,
            direction,
        );
        if let Some(start) = start {
            url.push_str(&format!("&start={}", urlencoding::encode(start)));
        }
        if let Some(end) = end {
            url.push_str(&format!("&end={}", urlencoding::encode(end)));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_log_labels(&self) -> Result<LokiLabelsResponse, reqwest::Error> {
        let req = self.client.get(format!("{}/logs/labels", self.base_url));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn entity_history(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<AuditEntry>, reqwest::Error> {
        let req = self.client.get(format!(
            "{}/audit/{}/{}",
            self.base_url, entity_type, entity_id
        ));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Git integration ──────────────────────────────────────

    pub async fn get_git_task_status(
        &self,
        task_id: Uuid,
    ) -> Result<GitTaskStatus, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/git/tasks/{}/status", self.base_url, task_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Git branch operations ────────────────────────────────

    pub async fn list_branches(
        &self,
        project_id: Uuid,
        prefix: Option<&str>,
    ) -> Result<BranchListResponse, reqwest::Error> {
        let mut url = format!("{}/{}/git/branches", self.base_url, project_id);
        if let Some(p) = prefix {
            url.push_str(&format!("?prefix={}", urlencoding::encode(p)));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn main_status(&self, project_id: Uuid) -> Result<MainPushStatus, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/git/main-status", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn push_main(&self, project_id: Uuid) -> Result<PushResponse, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/git/push-main", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn resolve_and_push_main(
        &self,
        project_id: Uuid,
    ) -> Result<PushResponse, reqwest::Error> {
        let req = self.client.post(format!(
            "{}/{}/git/resolve-and-push-main",
            self.base_url, project_id
        ));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn push_branch(
        &self,
        project_id: Uuid,
        branch: &str,
    ) -> Result<PushResponse, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/git/push", self.base_url, project_id))
            .json(&serde_json::json!({"branch": branch}));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Search ───────────────────────────────────────────────

    pub async fn search(
        &self,
        project_id: Uuid,
        q: &str,
        limit: Option<i64>,
    ) -> Result<SearchResponse, reqwest::Error> {
        let lim = limit.unwrap_or(50);
        let url = format!(
            "{}/{}/search?q={}&limit={}",
            self.base_url,
            project_id,
            urlencoding::encode(q),
            lim
        );
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Chat ─────────────────────────────────────────────────

    /// Stream chat SSE response, sending each chunk to the UI channel as it arrives.
    pub async fn send_chat_stream(
        &self,
        project_id: Uuid,
        messages: &[ChatMessage],
        model: Option<&str>,
        tx: &tokio::sync::mpsc::Sender<super::ApiMsg>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut body = serde_json::json!({"messages": messages});
        if let Some(m) = model {
            body["model"] = serde_json::json!(m);
        }
        let req = self
            .client
            .post(format!("{}/{}/chat", self.base_url, project_id))
            .json(&body);
        let resp = self.auth(req).send().await?.error_for_status()?;

        // Read the body incrementally via chunk()
        let mut buf = String::new();
        let mut resp = resp;
        while let Some(bytes) = resp.chunk().await? {
            let text = String::from_utf8_lossy(&bytes);
            buf.push_str(&text);

            // Process complete SSE lines from the buffer
            while let Some(newline_pos) = buf.find('\n') {
                let line = buf[..newline_pos].to_string();
                buf = buf[newline_pos + 1..].to_string();
                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(content) = obj.get("content").and_then(|c| c.as_str()) {
                            let _ = tx.send(super::ApiMsg::ChatChunk(content.to_string())).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // ── Source browser ───────────────────────────────────────

    pub async fn source_tree(
        &self,
        project_id: Uuid,
        path: &str,
        git_ref: Option<&str>,
    ) -> Result<TreeResponse, reqwest::Error> {
        let mut url = format!(
            "{}/{}/source/tree?path={}",
            self.base_url,
            project_id,
            urlencoding::encode(path)
        );
        if let Some(r) = git_ref {
            url.push_str(&format!("&ref={}", urlencoding::encode(r)));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn source_blob(
        &self,
        project_id: Uuid,
        path: &str,
        git_ref: Option<&str>,
    ) -> Result<BlobResponse, reqwest::Error> {
        let mut url = format!(
            "{}/{}/source/blob?path={}",
            self.base_url,
            project_id,
            urlencoding::encode(path)
        );
        if let Some(r) = git_ref {
            url.push_str(&format!("&ref={}", urlencoding::encode(r)));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Report operations ───────────────────────────────────────

    pub async fn list_reports(&self, project_id: Uuid) -> Result<Vec<Report>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/reports", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        // API returns PaginatedResponse, extract data field
        let body: serde_json::Value = resp.json().await?;
        Ok(serde_json::from_value(
            body.get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )
        .unwrap_or_default())
    }

    pub async fn create_report(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Report, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/reports", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn delete_report(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/reports/{}", self.base_url, id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Event operations ────────────────────────────────────────

    pub async fn list_events(
        &self,
        project_id: Uuid,
        kind: Option<&str>,
        severity: Option<&str>,
    ) -> Result<Vec<Event>, reqwest::Error> {
        let mut url = format!("{}/{}/events?limit=200", self.base_url, project_id);
        if let Some(k) = kind {
            url.push_str(&format!("&kind={}", k));
        }
        if let Some(s) = severity {
            url.push_str(&format!("&severity={}", s));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        let body: serde_json::Value = resp.json().await?;
        Ok(serde_json::from_value(
            body.get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![])),
        )
        .unwrap_or_default())
    }

    pub async fn create_event(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Event, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/events", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn list_recent_events(&self, project_id: Uuid) -> Result<Vec<Event>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/events/recent", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Webhook operations ──────────────────────────────────────

    pub async fn list_webhooks(&self, project_id: Uuid) -> Result<Vec<Webhook>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/webhooks", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn create_webhook(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Webhook, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/webhooks", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_webhook(
        &self,
        id: Uuid,
        body: serde_json::Value,
    ) -> Result<Webhook, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/webhooks/{}", self.base_url, id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn delete_webhook(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/webhooks/{}", self.base_url, id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn test_webhook(&self, id: Uuid) -> Result<serde_json::Value, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/webhooks/{}/test", self.base_url, id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_webhook_deliveries(
        &self,
        id: Uuid,
    ) -> Result<Vec<WebhookDelivery>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/webhooks/{}/deliveries", self.base_url, id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Step Template operations ────────────────────────────────

    // ── Task Log operations ─────────────────────────────────────

    // ── Metrics operations ──────────────────────────────────────

    pub async fn get_project_metrics(
        &self,
        project_id: Uuid,
        days: Option<i32>,
    ) -> Result<ProjectMetrics, reqwest::Error> {
        let mut url = format!("{}/{}/metrics", self.base_url, project_id);
        if let Some(d) = days {
            url.push_str(&format!("?days={}", d));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Work Comment operations ─────────────────────────────────

    pub async fn list_work_comments(
        &self,
        work_id: Uuid,
    ) -> Result<Vec<WorkComment>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/work/{}/comments", self.base_url, work_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Observation Cleanup operations ──────────────────────────

    pub async fn cleanup_observations(
        &self,
        project_id: Uuid,
    ) -> Result<CleanupObservationsResult, reqwest::Error> {
        let req = self.client.post(format!(
            "{}/{}/observations/cleanup",
            self.base_url, project_id
        ));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_agent_tasks(&self, agent_id: Uuid) -> Result<Vec<Task>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/agents/{}/tasks", self.base_url, agent_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }
}
