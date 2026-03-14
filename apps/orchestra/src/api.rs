use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde_json::Value;
use tracing::warn;

use crate::task_id::TaskId;

/// HTTP client for the Projects API.
#[derive(Clone)]
pub struct ProjectsApi {
    client: Client,
    base_url: String,
    agent_id: Option<String>,
    api_token: Option<String>,
    dev_user_id: Option<String>,
}

impl ProjectsApi {
    pub fn new(base_url: &str, agent_id: &str) -> Self {
        let api_token = std::env::var("DIRAIGENT_API_TOKEN")
            .ok()
            .filter(|t| !t.is_empty());
        let dev_user_id = std::env::var("DIRAIGENT_DEV_USER_ID")
            .ok()
            .filter(|t| !t.is_empty());
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("failed to build HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            agent_id: Some(agent_id.to_string()),
            api_token,
            dev_user_id,
        }
    }

    /// Create an API client without an agent ID (for setup).
    pub fn without_agent(base_url: &str) -> Self {
        let api_token = std::env::var("DIRAIGENT_API_TOKEN")
            .ok()
            .filter(|t| !t.is_empty());
        let dev_user_id = std::env::var("DIRAIGENT_DEV_USER_ID")
            .ok()
            .filter(|t| !t.is_empty());
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("failed to build HTTP client"),
            base_url: base_url.trim_end_matches('/').to_string(),
            agent_id: None,
            api_token,
            dev_user_id,
        }
    }

    pub fn agent_id(&self) -> &str {
        self.agent_id.as_deref().unwrap_or("")
    }

    /// Apply standard headers (Content-Type, X-Agent-Id, Authorization) to a request builder.
    fn build_request(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut req = req.header("Content-Type", "application/json");
        if let Some(ref agent_id) = self.agent_id {
            req = req.header("X-Agent-Id", agent_id);
        }
        if let Some(ref dev_user_id) = self.dev_user_id {
            // Dev mode: send X-Dev-User-Id header instead of a real JWT.
            req = req.header("X-Dev-User-Id", dev_user_id);
        } else if let Some(ref token) = self.api_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req
    }

    /// Bail with a descriptive error if the response status indicates failure.
    fn check_response(
        method: &str,
        url: &str,
        status: reqwest::StatusCode,
        body: &str,
    ) -> Result<()> {
        if status.is_client_error() || status.is_server_error() {
            bail!("{method} {url} → {status}: {body}");
        }
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.build_request(self.client.get(&url));
        let resp = req.send().await.with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Self::check_response("GET", &url, status, &body)?;
        serde_json::from_str(&body).with_context(|| format!("parse response from GET {url}"))
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.build_request(self.client.post(&url)).json(body);
        let resp = req.send().await.with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Self::check_response("POST", &url, status, &text)?;
        if text.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).with_context(|| format!("parse response from POST {url}"))
        }
    }

    async fn put(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);
        let req = self.build_request(self.client.put(&url)).json(body);
        let resp = req.send().await.with_context(|| format!("PUT {url}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Self::check_response("PUT", &url, status, &text)?;
        if text.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).with_context(|| format!("parse response from PUT {url}"))
        }
    }

    // ── Task operations ──────────────────────────────────────

    pub async fn get_task(&self, task_id: &str) -> Result<Value> {
        self.get(&format!("/tasks/{task_id}")).await
    }

    pub async fn get_ready_tasks(&self, project_id: &str) -> Result<Vec<Value>> {
        let val = self.get(&format!("/{project_id}/tasks/ready")).await?;
        Ok(as_array(&val))
    }

    pub async fn get_all_tasks(&self, project_id: &str) -> Result<Vec<Value>> {
        let val = self.get(&format!("/{project_id}/tasks")).await?;
        Ok(as_array(&val))
    }

    pub async fn claim_task(&self, task_id: &str) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/claim"),
            &serde_json::json!({"agent_id": self.agent_id}),
        )
        .await
    }

    pub async fn transition_task(&self, task_id: &str, state: &str) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/transition"),
            &serde_json::json!({"state": state}),
        )
        .await
    }

    /// Atomically transition a task to a new state and update its playbook_step.
    ///
    /// This is a convenience wrapper that performs `transition_task` followed by
    /// `update_task` to set the `playbook_step`. Both calls must succeed for the
    /// operation to be considered successful. If the transition succeeds but the
    /// update fails, the error from the update is returned.
    pub async fn transition_task_with_step(
        &self,
        task_id: &str,
        state: &str,
        playbook_step: u64,
    ) -> Result<Value> {
        self.transition_task(task_id, state).await?;
        self.update_task(
            task_id,
            &serde_json::json!({"playbook_step": playbook_step}),
        )
        .await
    }

    pub async fn update_task(&self, task_id: &str, body: &Value) -> Result<Value> {
        self.put(&format!("/tasks/{task_id}"), body).await
    }

    pub async fn post_task_update(
        &self,
        task_id: &str,
        kind: &str,
        content: &str,
    ) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/updates"),
            &serde_json::json!({"content": content, "kind": kind}),
        )
        .await
    }

    pub async fn get_task_updates(&self, task_id: &str) -> Result<Vec<Value>> {
        let val = self.get(&format!("/tasks/{task_id}/updates")).await?;
        Ok(as_array(&val))
    }

    pub async fn get_task_comments(&self, task_id: &str) -> Result<Vec<Value>> {
        let val = self.get(&format!("/tasks/{task_id}/comments")).await?;
        Ok(as_array(&val))
    }

    pub async fn create_task(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.post(&format!("/{project_id}/tasks"), body).await
    }

    pub async fn add_dependency(&self, task_id: &str, depends_on: &str) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/dependencies"),
            &serde_json::json!({"depends_on": depends_on}),
        )
        .await
    }

    pub async fn post_changed_files(
        &self,
        task_id: &str,
        files: &[crate::git::ChangedFile],
    ) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/changed-files"),
            &serde_json::json!({"files": files}),
        )
        .await
    }

    // ── Agent operations ─────────────────────────────────────

    pub async fn get_context(&self, project_id: &str) -> Result<Value> {
        let aid = self.agent_id();
        self.get(&format!("/agents/{aid}/context/{project_id}"))
            .await
    }

    /// Fetch agent context with semantic knowledge ranking for the given task.
    /// Passes `?task_id=<uuid>` so the API embeds the task spec and returns
    /// the top-k most relevant knowledge entries instead of the full list.
    pub async fn get_context_for_task(&self, project_id: &str, task_id: &str) -> Result<Value> {
        let aid = self.agent_id();
        self.get(&format!(
            "/agents/{aid}/context/{project_id}?task_id={task_id}"
        ))
        .await
    }

    pub async fn get_memberships(&self) -> Result<Vec<Value>> {
        let aid = self.agent_id();
        let val = self.get(&format!("/agents/{aid}/memberships")).await?;
        Ok(as_array(&val))
    }

    pub async fn heartbeat(&self) -> Result<Value> {
        let aid = self.agent_id();
        self.post(&format!("/agents/{aid}/heartbeat"), &serde_json::json!({}))
            .await
    }

    pub async fn update_agent(&self, agent_id: &str, body: &Value) -> Result<Value> {
        self.put(&format!("/agents/{agent_id}"), body).await
    }

    // ── Playbook operations ──────────────────────────────────

    pub async fn get_playbook(&self, playbook_id: &str) -> Result<Value> {
        self.get(&format!("/playbooks/{playbook_id}")).await
    }

    pub async fn list_playbooks(&self) -> Result<Vec<Value>> {
        let val = self.get("/playbooks").await?;
        Ok(as_array(&val))
    }

    pub async fn get_step_template(&self, template_id: &str) -> Result<Value> {
        self.get(&format!("/step-templates/{template_id}")).await
    }

    /// Record LLM token usage and cost for a task step. Values are accumulated
    /// on the task row so costs from multiple steps sum correctly.
    pub async fn post_task_cost(
        &self,
        task_id: &str,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/cost"),
            &serde_json::json!({
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "cost_usd": cost_usd,
            }),
        )
        .await
    }

    pub async fn post_comment(&self, task_id: &str, content: &str) -> Result<Value> {
        self.post(
            &format!("/tasks/{task_id}/comments"),
            &serde_json::json!({"content": content}),
        )
        .await
    }

    // ── Goal operations ───────────────────────────────────────

    pub async fn get_goals(&self, project_id: &str) -> Result<Vec<Value>> {
        let val = self.get(&format!("/{project_id}/goals")).await?;
        Ok(as_array(&val))
    }

    /// List goals filtered by status (e.g. "ready", "active", "processing").
    pub async fn list_goals_by_status(&self, project_id: &str, status: &str) -> Result<Vec<Value>> {
        let val = self
            .get(&format!("/{project_id}/goals?status={status}"))
            .await?;
        Ok(as_array(&val))
    }

    pub async fn get_goal(&self, goal_id: &str) -> Result<Value> {
        self.get(&format!("/goals/{goal_id}")).await
    }

    /// Update a goal's status (e.g. "processing", "active").
    pub async fn update_goal_status(&self, goal_id: &str, status: &str) -> Result<Value> {
        self.put(
            &format!("/goals/{goal_id}"),
            &serde_json::json!({"status": status}),
        )
        .await
    }

    pub async fn get_goal_progress(&self, goal_id: &str) -> Result<Value> {
        self.get(&format!("/goals/{goal_id}/progress")).await
    }

    /// Return all goals linked to a specific task (full goal objects).
    pub async fn get_task_goals(&self, task_id: &str) -> Result<Vec<Value>> {
        let val = self.get(&format!("/tasks/{task_id}/goals")).await?;
        Ok(as_array(&val))
    }

    /// Link a task to a goal.
    pub async fn link_task_to_goal(&self, goal_id: &str, task_id: &str) -> Result<Value> {
        self.post(
            &format!("/goals/{goal_id}/tasks"),
            &serde_json::json!({"task_id": task_id}),
        )
        .await
    }

    // ── Verification operations ────────────────────────────────

    pub async fn get_verifications(&self, project_id: &str, task_id: &str) -> Result<Vec<Value>> {
        let val = self
            .get(&format!(
                "/{project_id}/verifications?task_id={task_id}&limit=50"
            ))
            .await?;
        Ok(as_array(&val))
    }

    // ── Event operations ─────────────────────────────────────

    pub async fn post_event(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.post(&format!("/{project_id}/events"), body).await
    }

    // ── Task Log operations ────────────────────────────────────

    /// Upload a task execution log to the API.
    ///
    /// Uses a longer timeout (60s) since log content can be large (100KB+).
    pub async fn upload_task_log(
        &self,
        project_id: &str,
        task_id: &str,
        step_name: &str,
        content: &str,
        metadata: &Value,
    ) -> Result<Value> {
        let url = format!("{}/{project_id}/task-logs", self.base_url);
        let body = serde_json::json!({
            "task_id": task_id,
            "step_name": step_name,
            "content": content,
            "metadata": metadata,
        });
        let req = self
            .build_request(self.client.post(&url))
            .json(&body)
            .timeout(std::time::Duration::from_secs(60));
        let resp = req.send().await.with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Self::check_response("POST", &url, status, &text)?;
        if text.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).with_context(|| format!("parse response from POST {url}"))
        }
    }

    // ── Observation / Knowledge / Decision operations ────────

    pub async fn post_observation(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.post(&format!("/{project_id}/observations"), body)
            .await
    }

    pub async fn post_knowledge(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.post(&format!("/{project_id}/knowledge"), body).await
    }

    pub async fn post_decision(&self, project_id: &str, body: &Value) -> Result<Value> {
        self.post(&format!("/{project_id}/decisions"), body).await
    }

    // ── Related items operations ────────────────────────────

    /// Fetch related knowledge, decisions, and observations for a task.
    /// Returns a JSON object with `knowledge`, `decisions`, and `observations` arrays.
    pub async fn get_related_items(&self, task_id: &str) -> Result<Value> {
        self.get(&format!("/tasks/{task_id}/related")).await
    }

    // ── File lock operations ─────────────────────────────────

    /// Acquire file locks for a task. Returns Ok on success, Err on conflict (409)
    /// or other errors. The error message from a 409 contains details about which
    /// paths conflict with which existing locks.
    pub async fn acquire_file_locks(
        &self,
        project_id: &str,
        task_id: &str,
        paths: &[String],
    ) -> Result<Value> {
        let url = format!("{}/{project_id}/locks", self.base_url);
        let body = serde_json::json!({
            "task_id": task_id,
            "paths": paths,
        });
        let req = self.build_request(self.client.post(&url)).json(&body);
        let resp = req.send().await.with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Self::check_response("POST", &url, status, &text)?;
        if text.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).with_context(|| format!("parse response from POST {url}"))
        }
    }

    /// Release all file locks held by a task. Fire-and-forget pattern recommended
    /// by callers — log warnings on error but don't fail the operation.
    pub async fn release_file_locks(&self, project_id: &str, task_id: &str) -> Result<Value> {
        let url = format!("{}/{project_id}/locks/{task_id}", self.base_url);
        let req = self.build_request(self.client.delete(&url));
        let resp = req.send().await.with_context(|| format!("DELETE {url}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Self::check_response("DELETE", &url, status, &text)?;
        if text.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_str(&text).with_context(|| format!("parse response from DELETE {url}"))
        }
    }

    // ── Setup operations ─────────────────────────────────────

    /// Health check — GET {base_url}/../health/live (health is at server root).
    pub async fn health_check(&self) -> Result<()> {
        // base_url is e.g. http://localhost:8082/v1
        // health endpoint is at http://localhost:8082/health/live
        let base = &self.base_url;
        let server_root = base
            .find("/api/")
            .map(|i| &base[..i])
            .unwrap_or(base.trim_end_matches('/'));
        let url = format!("{server_root}/health/live");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            bail!("health check failed: {}", resp.status());
        }
        Ok(())
    }

    pub async fn register_agent(&self, body: &Value) -> Result<Value> {
        self.post("/agents", body).await
    }

    pub async fn list_projects(&self) -> Result<Vec<Value>> {
        let val = self.get("").await?;
        Ok(as_array(&val))
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Value> {
        self.get(&format!("/{project_id}")).await
    }

    pub async fn list_roles(&self) -> Result<Vec<Value>> {
        let val = self.get("/roles").await?;
        Ok(as_array(&val))
    }

    pub async fn add_member(&self, body: &Value) -> Result<Value> {
        self.post("/members", body).await
    }
}

fn as_array(val: &Value) -> Vec<Value> {
    match val {
        Value::Array(arr) => arr.clone(),
        Value::Object(obj) => {
            if let Some(Value::Array(arr)) = obj.get("data") {
                arr.clone()
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Retry an async operation up to 4 times with exponential backoff (1s, 2s, 4s).
/// Guards against transient API errors (503, network blips) that would otherwise
/// cause premature merge or stuck tasks. See observations 27a11e49 and f2c5f77c.
pub async fn retry_api_call<F, Fut, T>(op_name: &str, tid: &TaskId, f: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    const MAX_RETRIES: u32 = 4;
    const BACKOFF_SECS: [u64; 3] = [1, 2, 4];
    let mut last_err = None;
    for attempt in 1..=MAX_RETRIES {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                warn!("task {tid} {op_name} attempt {attempt}/{MAX_RETRIES} failed: {e}");
                last_err = Some(e);
                if attempt < MAX_RETRIES {
                    let delay = BACKOFF_SECS[attempt as usize - 1];
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("{op_name} failed after {MAX_RETRIES} retries")))
}
