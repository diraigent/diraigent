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
    pub priority: i32,
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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KnowledgeEntry {
    pub id: Uuid,
    pub title: String,
    pub category: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
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
pub struct Goal {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub goal_type: Option<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub parent_goal_id: Option<Uuid>,
    #[serde(default)]
    pub auto_status: Option<bool>,
    pub target_date: Option<String>,
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
pub struct GoalProgress {
    pub goal_id: Uuid,
    pub total_tasks: i64,
    pub done_tasks: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoalStats {
    pub goal_id: Uuid,
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
pub struct Playbook {
    pub id: Uuid,
    pub title: String,
    pub trigger_description: Option<String>,
    #[serde(default)]
    pub steps: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
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
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub authorities: Vec<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub knowledge_scope: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Member {
    pub id: Uuid,
    #[serde(default)]
    pub agent_id: Option<Uuid>,
    #[serde(default)]
    pub role_id: Option<Uuid>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
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

// ── Verification types ────────────────────────────────────
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Verification {
    pub id: Uuid,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    pub kind: String,
    pub status: String,
    pub title: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub evidence: Option<serde_json::Value>,
    #[serde(default)]
    pub created_by_agent_id: Option<Uuid>,
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

    pub async fn post_comment(&self, task_id: Uuid, content: &str) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/tasks/{}/comments", self.base_url, task_id))
            .json(&serde_json::json!({"content": content}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn transition_task(&self, task_id: Uuid, state: &str) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/tasks/{}/transition", self.base_url, task_id))
            .json(&serde_json::json!({"state": state}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn post_update(
        &self,
        task_id: Uuid,
        content: &str,
        kind: &str,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/tasks/{}/updates", self.base_url, task_id))
            .json(&serde_json::json!({"content": content, "kind": kind}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn create_task(
        &self,
        project_id: Uuid,
        title: &str,
        kind: &str,
        priority: u8,
        spec: &str,
        playbook_id: Option<Uuid>,
    ) -> Result<Task, reqwest::Error> {
        let mut body = serde_json::json!({
            "title": title,
            "kind": kind,
            "priority": priority,
            "context": { "spec": spec }
        });
        if let Some(pid) = playbook_id {
            body["playbook_id"] = serde_json::json!(pid);
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

    pub async fn list_agents(&self) -> Result<Vec<Agent>, reqwest::Error> {
        let req = self.client.get(format!("{}/agents", self.base_url));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_knowledge(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<KnowledgeEntry>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/knowledge", self.base_url, project_id));
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

    pub async fn list_playbooks(&self, project_id: Uuid) -> Result<Vec<Playbook>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/playbooks", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Goal operations ──────────────────────────────────────

    pub async fn list_goals(&self, project_id: Uuid) -> Result<Vec<Goal>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/{}/goals", self.base_url, project_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn get_goal_progress(&self, goal_id: Uuid) -> Result<GoalProgress, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/goals/{}/progress", self.base_url, goal_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn get_goal_stats(&self, goal_id: Uuid) -> Result<GoalStats, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/goals/{}/stats", self.base_url, goal_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn list_goal_children(&self, goal_id: Uuid) -> Result<Vec<Goal>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/goals/{}/children", self.base_url, goal_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_goal(
        &self,
        project_id: Uuid,
        title: &str,
        description: &str,
        goal_type: &str,
        priority: i32,
        parent_goal_id: Option<Uuid>,
        auto_status: bool,
    ) -> Result<Goal, reqwest::Error> {
        let mut body = serde_json::json!({
            "title": title,
            "description": description,
            "goal_type": goal_type,
            "priority": priority,
            "auto_status": auto_status,
        });
        if let Some(pid) = parent_goal_id {
            body["parent_goal_id"] = serde_json::json!(pid);
        }
        let req = self
            .client
            .post(format!("{}/{}/goals", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_goal(
        &self,
        goal_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Goal, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/goals/{}", self.base_url, goal_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn link_task_to_goal(
        &self,
        goal_id: Uuid,
        task_id: Uuid,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/goals/{}/tasks", self.base_url, goal_id))
            .json(&serde_json::json!({"task_id": task_id}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn unlink_task_from_goal(
        &self,
        goal_id: Uuid,
        task_id: Uuid,
    ) -> Result<(), reqwest::Error> {
        let req = self.client.delete(format!(
            "{}/goals/{}/tasks/{}",
            self.base_url, goal_id, task_id
        ));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn list_goal_tasks(
        &self,
        goal_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, reqwest::Error> {
        let req = self.client.get(format!(
            "{}/goals/{}/tasks?limit={}&offset={}",
            self.base_url, goal_id, limit, offset
        ));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn bulk_link_tasks(
        &self,
        goal_id: Uuid,
        task_ids: &[Uuid],
    ) -> Result<serde_json::Value, reqwest::Error> {
        let body = serde_json::json!({ "task_ids": task_ids });
        let req = self
            .client
            .post(format!("{}/goals/{}/tasks/bulk", self.base_url, goal_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn list_unlinked_tasks(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, reqwest::Error> {
        let req = self.client.get(format!(
            "{}/{}/tasks?unlinked=true&limit={}&offset={}",
            self.base_url, project_id, limit, offset
        ));
        let resp = self.auth(req).send().await?.error_for_status()?;
        // The endpoint may return paginated or plain array; try paginated first
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
        priority: u8,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/observations/{}/promote", self.base_url, id))
            .json(&serde_json::json!({"title": title, "kind": kind, "priority": priority}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Knowledge/Decision create & edit operations ─────────────────────

    pub async fn create_knowledge(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<KnowledgeEntry, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/knowledge", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

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

    pub async fn update_knowledge(
        &self,
        id: Uuid,
        body: serde_json::Value,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/knowledge/{}", self.base_url, id));
        self.auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn delete_knowledge(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/knowledge/{}", self.base_url, id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
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

    pub async fn remove_dependency(
        &self,
        task_id: Uuid,
        dep_id: Uuid,
    ) -> Result<(), reqwest::Error> {
        let req = self.client.delete(format!(
            "{}/tasks/{}/dependencies/{}",
            self.base_url, task_id, dep_id
        ));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Playbook CRUD operations ──────────────────────────────

    pub async fn create_playbook(
        &self,
        project_id: Uuid,
        title: &str,
        trigger: &str,
        steps: serde_json::Value,
        tags: Vec<String>,
    ) -> Result<Playbook, reqwest::Error> {
        let body = serde_json::json!({
            "title": title,
            "trigger_description": trigger,
            "steps": steps,
            "tags": tags,
        });
        let req = self
            .client
            .post(format!("{}/{}/playbooks", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_playbook(
        &self,
        id: Uuid,
        body: serde_json::Value,
    ) -> Result<Playbook, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/playbooks/{}", self.base_url, id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn delete_playbook(&self, id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/playbooks/{}", self.base_url, id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Role operations ───────────────────────────────────────

    pub async fn list_roles(&self) -> Result<Vec<Role>, reqwest::Error> {
        let req = self.client.get(format!("{}/roles", self.base_url));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn create_role(&self, body: serde_json::Value) -> Result<Role, reqwest::Error> {
        let req = self.client.post(format!("{}/roles", self.base_url));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_role(
        &self,
        role_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Role, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/roles/{}", self.base_url, role_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn delete_role(&self, role_id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/roles/{}", self.base_url, role_id));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    // ── Member operations ─────────────────────────────────────

    pub async fn list_members(&self) -> Result<Vec<Member>, reqwest::Error> {
        let req = self.client.get(format!("{}/members", self.base_url));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn create_member(&self, body: serde_json::Value) -> Result<Member, reqwest::Error> {
        let req = self.client.post(format!("{}/members", self.base_url));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_member(
        &self,
        member_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Member, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/members/{}", self.base_url, member_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn delete_member(&self, member_id: Uuid) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .delete(format!("{}/members/{}", self.base_url, member_id));
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

    // ── Verification operations ────────────────────────────────

    pub async fn list_verifications(
        &self,
        project_id: Uuid,
        task_id: Option<Uuid>,
        kind: Option<&str>,
        status: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Verification>, reqwest::Error> {
        let mut url = format!(
            "{}/{}/verifications?limit={}&offset={}",
            self.base_url, project_id, limit, offset,
        );
        if let Some(tid) = task_id {
            url.push_str(&format!("&task_id={}", tid));
        }
        if let Some(k) = kind {
            url.push_str(&format!("&kind={}", urlencoding::encode(k)));
        }
        if let Some(s) = status {
            url.push_str(&format!("&status={}", urlencoding::encode(s)));
        }
        let req = self.client.get(&url);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn get_verification(&self, id: Uuid) -> Result<Verification, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/verifications/{}", self.base_url, id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn create_verification(
        &self,
        project_id: Uuid,
        body: serde_json::Value,
    ) -> Result<Verification, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/{}/verifications", self.base_url, project_id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
    }

    pub async fn update_verification(
        &self,
        id: Uuid,
        body: serde_json::Value,
    ) -> Result<Verification, reqwest::Error> {
        let req = self
            .client
            .put(format!("{}/verifications/{}", self.base_url, id));
        let resp = self
            .auth(req)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        resp.json().await
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

    pub async fn list_log_label_values(
        &self,
        label: &str,
    ) -> Result<LokiLabelsResponse, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/logs/labels/{}/values", self.base_url, label));
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

    // ── Task management operations ───────────────────────────

    pub async fn claim_task(
        &self,
        task_id: Uuid,
        agent_id: Option<Uuid>,
    ) -> Result<Task, reqwest::Error> {
        let mut body = serde_json::json!({});
        if let Some(aid) = agent_id {
            body["agent_id"] = serde_json::json!(aid);
        }
        let req = self
            .client
            .post(format!("{}/tasks/{}/claim", self.base_url, task_id))
            .json(&body);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn release_task(&self, task_id: Uuid) -> Result<Task, reqwest::Error> {
        let req = self
            .client
            .post(format!("{}/tasks/{}/release", self.base_url, task_id))
            .json(&serde_json::json!({}));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    pub async fn delegate_task(
        &self,
        task_id: Uuid,
        agent_id: Option<Uuid>,
        role_id: Option<Uuid>,
    ) -> Result<Task, reqwest::Error> {
        let mut body = serde_json::json!({});
        if let Some(aid) = agent_id {
            body["agent_id"] = serde_json::json!(aid);
        }
        if let Some(rid) = role_id {
            body["role_id"] = serde_json::json!(rid);
        }
        let req = self
            .client
            .post(format!("{}/tasks/{}/delegate", self.base_url, task_id))
            .json(&body);
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }

    // ── Bulk operations ──────────────────────────────────────

    pub async fn bulk_transition(
        &self,
        project_id: Uuid,
        task_ids: Vec<Uuid>,
        state: &str,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!(
                "{}/{}/tasks/bulk/transition",
                self.base_url, project_id
            ))
            .json(&serde_json::json!({"task_ids": task_ids, "state": state}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn bulk_delete(
        &self,
        project_id: Uuid,
        task_ids: Vec<Uuid>,
    ) -> Result<(), reqwest::Error> {
        let req = self
            .client
            .post(format!(
                "{}/{}/tasks/bulk/delete",
                self.base_url, project_id
            ))
            .json(&serde_json::json!({"task_ids": task_ids}));
        self.auth(req).send().await?.error_for_status()?;
        Ok(())
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

    pub async fn get_changed_files(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<ChangedFile>, reqwest::Error> {
        let req = self
            .client
            .get(format!("{}/tasks/{}/changed-files", self.base_url, task_id));
        let resp = self.auth(req).send().await?.error_for_status()?;
        resp.json().await
    }
}
