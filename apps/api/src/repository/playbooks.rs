use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

/// Validate that all `step_template_id` references in a playbook's steps array
/// point to existing step templates. Returns a validation error listing any
/// invalid IDs. Called on playbook create/update.
pub async fn validate_step_template_ids(
    pool: &PgPool,
    steps: &serde_json::Value,
) -> Result<(), AppError> {
    let Some(steps_array) = steps.as_array() else {
        return Ok(());
    };

    let template_ids: Vec<Uuid> = steps_array
        .iter()
        .filter_map(|step| step["step_template_id"].as_str())
        .filter_map(|id_str| Uuid::parse_str(id_str).ok())
        .collect();

    if template_ids.is_empty() {
        return Ok(());
    }

    let existing: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM diraigent.step_template WHERE id = ANY($1)")
            .bind(&template_ids)
            .fetch_all(pool)
            .await?;

    let missing: Vec<String> = template_ids
        .iter()
        .filter(|id| !existing.contains(id))
        .map(|id| id.to_string())
        .collect();

    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "step_template_id references not found: {}",
            missing.join(", ")
        )));
    }

    Ok(())
}

/// Resolve a playbook's steps by merging step template defaults with inline overrides.
pub async fn resolve_playbook_steps(pool: &PgPool, steps: &serde_json::Value) -> serde_json::Value {
    let Some(steps_array) = steps.as_array() else {
        return steps.clone();
    };

    let template_ids: Vec<Uuid> = steps_array
        .iter()
        .filter_map(|step| step["step_template_id"].as_str())
        .filter_map(|id_str| Uuid::parse_str(id_str).ok())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let templates: std::collections::HashMap<Uuid, StepTemplate> = if !template_ids.is_empty() {
        let rows: Vec<StepTemplate> =
            sqlx::query_as("SELECT * FROM diraigent.step_template WHERE id = ANY($1)")
                .bind(&template_ids)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        rows.into_iter().map(|t| (t.id, t)).collect()
    } else {
        std::collections::HashMap::new()
    };

    let resolved: Vec<serde_json::Value> = steps_array
        .iter()
        .map(|step| {
            let template_id = step["step_template_id"]
                .as_str()
                .and_then(|id_str| Uuid::parse_str(id_str).ok());

            let Some(tid) = template_id else {
                return step.clone();
            };

            let Some(template) = templates.get(&tid) else {
                warn!(
                    "step_template_id {} not found — using inline properties only",
                    tid
                );
                return step.clone();
            };

            merge_template_with_step(template, step)
        })
        .collect();

    serde_json::Value::Array(resolved)
}

fn merge_template_with_step(
    template: &StepTemplate,
    inline_step: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = serde_json::Map::new();

    merged.insert("name".to_string(), serde_json::json!(template.name));
    if let Some(ref v) = template.description {
        merged.insert("description".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.model {
        merged.insert("model".to_string(), serde_json::json!(v));
    }
    if let Some(v) = template.budget {
        merged.insert("budget".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.allowed_tools {
        merged.insert("allowed_tools".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.context_level {
        merged.insert("context_level".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.on_complete {
        merged.insert("on_complete".to_string(), serde_json::json!(v));
    }
    if let Some(v) = template.retriable {
        merged.insert("retriable".to_string(), serde_json::json!(v));
    }
    if let Some(v) = template.max_cycles {
        merged.insert("max_cycles".to_string(), serde_json::json!(v));
    }
    if let Some(v) = template.timeout_minutes {
        merged.insert("timeout_minutes".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.mcp_servers {
        merged.insert("mcp_servers".to_string(), v.clone());
    }
    if let Some(ref v) = template.agents {
        merged.insert("agents".to_string(), v.clone());
    }
    if let Some(ref v) = template.agent {
        merged.insert("agent".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.settings {
        merged.insert("settings".to_string(), v.clone());
    }
    if let Some(ref v) = template.env {
        merged.insert("env".to_string(), v.clone());
    }
    if let Some(ref v) = template.vars {
        merged.insert("vars".to_string(), v.clone());
    }
    if let Some(ref v) = template.provider {
        merged.insert("provider".to_string(), serde_json::json!(v));
    }
    if let Some(ref v) = template.base_url {
        merged.insert("base_url".to_string(), serde_json::json!(v));
    }

    if let Some(obj) = inline_step.as_object() {
        for (key, value) in obj {
            if key == "step_template_id" {
                merged.insert(key.clone(), value.clone());
                continue;
            }
            if !value.is_null() {
                merged.insert(key.clone(), value.clone());
            }
        }
    }

    serde_json::Value::Object(merged)
}
