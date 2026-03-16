use crate::project::api::ProjectsApi;
use crate::providers::{ProviderConfig, ProviderFactory, ResolvedStep, TaskContext};
use crate::ws::WsSender;
use crate::ws::protocol::WsMessage;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info, warn};

/// Parameters for a plan request.
pub struct PlanRequestParams {
    pub sender: WsSender,
    pub request_id: String,
    pub title: String,
    pub description: String,
    pub success_criteria: serde_json::Value,
    pub project_name: String,
    pub project_id: String,
    pub api: ProjectsApi,
    pub projects_path: PathBuf,
    /// Optional model override from the client/project metadata.
    pub model: Option<String>,
}

/// Handle a plan.request by decomposing a work item into concrete tasks.
///
/// Reads `metadata.plan_provider` from the project to determine which provider
/// to use.  All providers are dispatched through the [`ProviderFactory`]
/// abstraction — no provider-specific code paths exist here.
pub async fn handle_plan_request(params: PlanRequestParams) {
    let PlanRequestParams {
        sender,
        request_id,
        title,
        description,
        success_criteria,
        project_name,
        project_id,
        api,
        projects_path,
        model: model_override,
    } = params;

    let send_error = |sender: WsSender, request_id: String, msg: String| async move {
        let ws_msg = WsMessage::PlanResponse {
            request_id,
            success: false,
            error: Some(msg),
            tasks: serde_json::Value::Array(vec![]),
        };
        if let Err(e) = sender.send(ws_msg) {
            error!("failed to send plan error via WS: {e}");
        }
    };

    // Resolve the project's working directory from the API.
    let working_dir = crate::project::paths::resolve_working_dir(&api, &project_id, &projects_path)
        .await
        .unwrap_or_else(|e| {
            warn!(
                project_id = %project_id,
                error = %e,
                "failed to resolve project working dir, falling back to projects_path"
            );
            projects_path.clone()
        });

    // Build planning prompt
    let criteria_text = match &success_criteria {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(|s| format!("- {s}")))
            .collect::<Vec<_>>()
            .join("\n"),
        serde_json::Value::String(s) => s.clone(),
        _ => success_criteria.to_string(),
    };

    let desc = if description.is_empty() {
        "No description provided"
    } else {
        &description
    };
    let criteria = if criteria_text.is_empty() {
        "None specified".to_string()
    } else {
        criteria_text
    };

    let system_prompt = format!(
        "You are a technical project planner for the project \"{project_name}\". \
         You can read files in the codebase to understand the project structure before planning. \
         Your final output MUST be a JSON object matching this schema (no markdown fences, no preamble):\n\
         {{\"tasks\": [{{\"title\": \"...\", \"kind\": \"feature|bug|refactor|test|docs\", \"spec\": \"...\", \"acceptance_criteria\": [\"...\"], \"depends_on\": [0]}}]}}"
    );

    let prompt = format!(
        r#"Decompose the following work item into 3-8 concrete, implementable tasks. Each task should be small enough for a single developer to complete in one session.

## Work Item
**Title**: {title}
**Description**: {desc}
**Success Criteria**:
{criteria}

## Requirements
- Order tasks by dependency (tasks that must be done first come first)
- Each task must have a clear, specific scope
- kind must be one of: feature, bug, refactor, test, docs
- spec should be a concise technical description of what to implement (2-4 sentences, not a full design doc)
- acceptance_criteria should be verifiable conditions (not vague)
- depends_on is an array of zero-based indices referencing earlier tasks in this list that must complete first
- The first task must always have depends_on: [] (empty array)
- Do NOT create meta-tasks like "review" or "deploy" — only implementation work

Respond with ONLY a JSON object matching this schema (no markdown fences, no preamble):
{{"tasks": [{{"title": "...", "kind": "feature|bug|refactor|test|docs", "spec": "...", "acceptance_criteria": ["..."], "depends_on": [0]}}]}}"#
    );

    // Resolve provider and model from project metadata (default: "claude-code")
    let (provider_name, metadata_model) = match api.get_project(&project_id).await {
        Ok(project) => {
            let provider = project["metadata"]["plan_provider"]
                .as_str()
                .unwrap_or("claude-code")
                .to_string();
            let model = project["metadata"]["plan_model"]
                .as_str()
                .map(|s| s.to_string());
            (provider, model)
        }
        Err(e) => {
            warn!("plan request: failed to fetch project metadata: {e}, using default provider");
            ("claude-code".to_string(), None)
        }
    };

    // Model priority: client override > project metadata > default
    let resolved_model = model_override
        .filter(|m| !m.is_empty())
        .or(metadata_model)
        .unwrap_or_else(|| "sonnet".to_string());

    info!("plan request: using provider '{provider_name}' (model={resolved_model})");

    // Create provider via factory
    let provider = match ProviderFactory::create(&provider_name) {
        Ok(p) => p,
        Err(e) => {
            send_error(
                sender,
                request_id,
                format!("Unknown provider '{provider_name}': {e}"),
            )
            .await;
            return;
        }
    };

    // Resolve credentials from API
    let provider_cfg = match api
        .resolve_provider_config(&project_id, &provider_name)
        .await
    {
        Ok(cfg) => ProviderConfig {
            api_key: cfg["api_key"].as_str().map(String::from),
            base_url: cfg["base_url"].as_str().map(String::from),
            model: cfg["default_model"].as_str().map(String::from),
        },
        Err(e) => {
            warn!("plan request: no provider config for '{provider_name}': {e}");
            ProviderConfig {
                api_key: None,
                base_url: None,
                model: None,
            }
        }
    };

    // Build a temporary log file for providers that need it (e.g. CLI-based).
    let log_dir = std::env::temp_dir().join("diraigent-plan");
    let _ = tokio::fs::create_dir_all(&log_dir).await;
    let log_file = log_dir.join(format!("plan-{request_id}.jsonl"));

    let step = ResolvedStep {
        name: "plan".into(),
        description: "Technical project planner. Respond with valid JSON only.".into(),
        model: Some(
            provider_cfg
                .model
                .clone()
                .unwrap_or_else(|| resolved_model.clone()),
        ),
        allowed_tools: None,
        allowed_tools_list: vec![],
        budget: None,
        env: HashMap::new(),
        system_prompt: Some(system_prompt),
        mcp_servers: None,
        agents: None,
        agent: None,
        settings: None,
    };

    let task_ctx = TaskContext {
        task_id: format!("plan-{request_id}"),
        project_id: project_id.to_string(),
        project_context: String::new(),
        previous_step_output: None,
        working_dir: Some(working_dir),
        log_file: Some(log_file.clone()),
        user_prompt: Some(prompt),
    };

    let output = match provider.execute(&step, &task_ctx, &provider_cfg).await {
        Ok(output) => output,
        Err(e) => {
            send_error(sender, request_id, format!("Provider error: {e}")).await;
            // Clean up temp log file
            let _ = tokio::fs::remove_file(&log_file).await;
            return;
        }
    };

    // Clean up temp log file
    let _ = tokio::fs::remove_file(&log_file).await;

    if output.is_error {
        send_error(sender, request_id, output.content).await;
        return;
    }

    if output.content.trim().is_empty() {
        send_error(
            sender,
            request_id,
            "Provider returned empty response".into(),
        )
        .await;
        return;
    }

    info!(
        input_tokens = output.input_tokens,
        output_tokens = output.output_tokens,
        "plan request: provider completed"
    );

    send_plan_result(sender, request_id, &output.content);
}

/// Parse JSON from the AI response text and send the plan result via WS.
fn send_plan_result(sender: WsSender, request_id: String, text: &str) {
    let text = text.trim();
    let json_text = extract_json_object(text);

    let parsed: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                error = %e,
                text = %json_text,
                "failed to parse plan response as JSON"
            );
            let preview: String = json_text.chars().take(200).collect();
            let ws_msg = WsMessage::PlanResponse {
                request_id,
                success: false,
                error: Some(format!(
                    "Failed to parse AI response as JSON: {e} (preview: {preview})"
                )),
                tasks: serde_json::Value::Array(vec![]),
            };
            if let Err(e) = sender.send(ws_msg) {
                error!("failed to send plan error via WS: {e}");
            }
            return;
        }
    };

    // Extract the tasks array
    let tasks = if let Some(tasks) = parsed.get("tasks") {
        tasks.clone()
    } else if parsed.is_array() {
        parsed
    } else {
        let ws_msg = WsMessage::PlanResponse {
            request_id,
            success: false,
            error: Some("AI response did not contain a 'tasks' array".into()),
            tasks: serde_json::Value::Array(vec![]),
        };
        if let Err(e) = sender.send(ws_msg) {
            error!("failed to send plan error via WS: {e}");
        }
        return;
    };

    let ws_msg = WsMessage::PlanResponse {
        request_id,
        success: true,
        error: None,
        tasks,
    };

    if let Err(e) = sender.send(ws_msg) {
        error!("failed to send plan response via WS: {e}");
    }

    info!("plan request completed");
}

/// Extract a JSON object from text that may contain preamble or markdown fences.
fn extract_json_object(text: &str) -> &str {
    // 1. Already valid JSON — fast path
    if text.starts_with('{') || text.starts_with('[') {
        return text;
    }

    // 2. Strip markdown fences: ```json ... ``` or ``` ... ```
    if let Some(fence_start) = text.find("```") {
        let after_fence = &text[fence_start + 3..];
        let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
        let inner = &after_fence[content_start..];
        if let Some(end) = inner.find("```") {
            let candidate = inner[..end].trim();
            if candidate.starts_with('{') || candidate.starts_with('[') {
                return candidate;
            }
        }
    }

    // 3. Find the outermost { ... } substring
    if let Some(start) = text.find('{')
        && let Some(end) = text.rfind('}')
        && end > start
    {
        return &text[start..=end];
    }

    // 4. Fallback
    text
}
