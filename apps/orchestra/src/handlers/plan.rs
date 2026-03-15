use crate::project::api::ProjectsApi;
use crate::providers::{ProviderConfig, ProviderFactory, ResolvedStep, TaskContext};
use crate::ws::WsSender;
use crate::ws::protocol::WsMessage;
use std::collections::HashMap;
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
}

/// Handle a plan.request by calling an AI provider to decompose a work item
/// into concrete tasks.
///
/// Uses the provider abstraction instead of spawning the Claude CLI directly,
/// making plan requests model-agnostic.
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

    let prompt = format!(
        r#"You are a technical project planner for the project "{project_name}".

Decompose the following work item into 3-8 concrete, implementable tasks. Each task should be small enough for a single developer to complete in one session.

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

    // Resolve provider — try "anthropic" first, fall back to any configured provider
    let provider_name = "anthropic";
    let provider = match ProviderFactory::create(provider_name) {
        Ok(p) => p,
        Err(e) => {
            send_error(
                sender,
                request_id,
                format!("Failed to create provider: {e}"),
            )
            .await;
            return;
        }
    };

    // Resolve credentials from API
    let provider_cfg = match api
        .resolve_provider_config(&project_id, provider_name)
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

    let step = ResolvedStep {
        name: "plan".into(),
        description: "You are a technical project planner. Respond with valid JSON only.".into(),
        model: Some(
            provider_cfg
                .model
                .clone()
                .unwrap_or_else(|| "claude-sonnet-4-6-20250514".into()),
        ),
        allowed_tools: None,
        allowed_tools_list: vec![],
        budget: None,
        env: HashMap::new(),
        system_prompt: None,
        mcp_servers: None,
        agents: None,
        agent: None,
        settings: None,
    };

    let task_ctx = TaskContext {
        task_id: format!("plan-{}", &request_id),
        project_id: project_id.to_string(),
        project_context: String::new(),
        previous_step_output: None,
        working_dir: None,
        log_file: None,
        user_prompt: Some(prompt),
    };

    info!("plan request: calling provider '{provider_name}'");

    let output = match provider.execute(&step, &task_ctx, &provider_cfg).await {
        Ok(output) => output,
        Err(e) => {
            send_error(sender, request_id, format!("Provider error: {e}")).await;
            return;
        }
    };

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

    // Parse JSON response (may have preamble or markdown fences)
    let text = output.content.trim();
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
            send_error(
                sender,
                request_id,
                format!("Failed to parse AI response as JSON: {e} (preview: {preview})"),
            )
            .await;
            return;
        }
    };

    // Extract the tasks array
    let tasks = if let Some(tasks) = parsed.get("tasks") {
        tasks.clone()
    } else if parsed.is_array() {
        parsed
    } else {
        send_error(
            sender,
            request_id,
            "AI response did not contain a 'tasks' array".into(),
        )
        .await;
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
