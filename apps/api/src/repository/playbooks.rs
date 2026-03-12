use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id};

// ── Playbooks ──

pub async fn create_playbook(
    pool: &PgPool,
    tenant_id: Uuid,
    req: &CreatePlaybook,
    created_by: Uuid,
) -> Result<Playbook, AppError> {
    let steps = req.steps.clone().unwrap_or(serde_json::json!([]));
    let tags = req.tags.clone().unwrap_or_default();
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));
    let initial_state = req.initial_state.as_deref().unwrap_or("ready");

    let p = sqlx::query_as::<_, Playbook>(
        "INSERT INTO diraigent.playbook (tenant_id, title, trigger_description, steps, tags, metadata, initial_state, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
    )
    .bind(tenant_id)
    .bind(&req.title)
    .bind(&req.trigger_description)
    .bind(&steps)
    .bind(&tags)
    .bind(&metadata)
    .bind(initial_state)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(p)
}

pub async fn get_playbook_by_id(pool: &PgPool, id: Uuid) -> Result<Playbook, AppError> {
    fetch_by_id(pool, Table::Playbook, id, "Playbook not found").await
}

pub async fn list_playbooks(
    pool: &PgPool,
    tenant_id: Uuid,
    filters: &PlaybookFilters,
) -> Result<Vec<Playbook>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    // Return playbooks that belong to this tenant OR are shared (tenant_id IS NULL)
    let items = sqlx::query_as::<_, Playbook>(
        "SELECT * FROM diraigent.playbook
         WHERE (tenant_id = $1 OR tenant_id IS NULL)
           AND ($2::text IS NULL OR $2 = ANY(tags))
         ORDER BY created_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(tenant_id)
    .bind(&filters.tag)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn update_playbook(
    pool: &PgPool,
    id: Uuid,
    req: &UpdatePlaybook,
) -> Result<Playbook, AppError> {
    let existing = get_playbook_by_id(pool, id).await?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let trigger_description = req
        .trigger_description
        .as_deref()
        .or(existing.trigger_description.as_deref());
    let steps = req.steps.as_ref().unwrap_or(&existing.steps);
    let tags = req.tags.as_ref().unwrap_or(&existing.tags);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);
    let initial_state = req
        .initial_state
        .as_deref()
        .unwrap_or(&existing.initial_state);

    let p = sqlx::query_as::<_, Playbook>(
        "UPDATE diraigent.playbook
         SET title = $2, trigger_description = $3, steps = $4, tags = $5,
             metadata = $6, initial_state = $7, version = version + 1
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(trigger_description)
    .bind(steps)
    .bind(tags)
    .bind(metadata)
    .bind(initial_state)
    .fetch_one(pool)
    .await?;

    Ok(p)
}

/// Fork a default (tenant_id = NULL) playbook into a tenant-owned copy,
/// applying any fields from `req` on top of the source.
/// Records parent_id and parent_version so the fork can detect upstream updates.
pub async fn fork_playbook(
    pool: &PgPool,
    tenant_id: Uuid,
    source: &Playbook,
    req: &UpdatePlaybook,
    created_by: Uuid,
) -> Result<Playbook, AppError> {
    let title = req.title.as_deref().unwrap_or(&source.title);
    let trigger_description = req
        .trigger_description
        .as_deref()
        .or(source.trigger_description.as_deref());
    let steps = req.steps.as_ref().unwrap_or(&source.steps);
    let tags = req.tags.as_ref().unwrap_or(&source.tags);
    let metadata = req.metadata.as_ref().unwrap_or(&source.metadata);
    let initial_state = req
        .initial_state
        .as_deref()
        .unwrap_or(&source.initial_state);

    let p = sqlx::query_as::<_, Playbook>(
        "INSERT INTO diraigent.playbook
         (tenant_id, title, trigger_description, steps, tags, metadata, initial_state, created_by,
          parent_id, parent_version)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *",
    )
    .bind(tenant_id)
    .bind(title)
    .bind(trigger_description)
    .bind(steps)
    .bind(tags)
    .bind(metadata)
    .bind(initial_state)
    .bind(created_by)
    .bind(source.id)
    .bind(source.version)
    .fetch_one(pool)
    .await?;

    Ok(p)
}

/// Re-sync a forked playbook with its parent's latest content.
/// Copies steps, metadata, tags etc. from the parent, bumps version, and
/// updates parent_version to the parent's current version.
pub async fn sync_playbook_with_parent(pool: &PgPool, id: Uuid) -> Result<Playbook, AppError> {
    let fork = get_playbook_by_id(pool, id).await?;
    let parent_id = fork
        .parent_id
        .ok_or_else(|| AppError::Validation("playbook has no parent to sync from".into()))?;
    let parent = get_playbook_by_id(pool, parent_id).await?;

    let p = sqlx::query_as::<_, Playbook>(
        "UPDATE diraigent.playbook
         SET steps = $2, tags = $3, metadata = $4, initial_state = $5,
             trigger_description = $6, parent_version = $7, version = version + 1
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&parent.steps)
    .bind(&parent.tags)
    .bind(&parent.metadata)
    .bind(&parent.initial_state)
    .bind(&parent.trigger_description)
    .bind(parent.version)
    .fetch_one(pool)
    .await?;

    Ok(p)
}

pub async fn delete_playbook(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Playbook, id, "Playbook not found").await
}

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

    // Check which template IDs exist in the database
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
///
/// For each step in the steps array:
/// - If `step_template_id` is present: fetch the template, use template values as defaults,
///   overlay any inline properties from the step (inline wins).
/// - If no `step_template_id`: use inline properties as-is (backward compatible).
///
/// If a referenced template no longer exists, falls back to inline properties with a warning.
pub async fn resolve_playbook_steps(pool: &PgPool, steps: &serde_json::Value) -> serde_json::Value {
    let Some(steps_array) = steps.as_array() else {
        return steps.clone();
    };

    // Collect all unique template IDs to batch-fetch
    let template_ids: Vec<Uuid> = steps_array
        .iter()
        .filter_map(|step| step["step_template_id"].as_str())
        .filter_map(|id_str| Uuid::parse_str(id_str).ok())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Batch-fetch all needed templates
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

/// Merge a step template's properties as defaults, with the inline step's properties
/// taking precedence (inline wins).
fn merge_template_with_step(
    template: &StepTemplate,
    inline_step: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = serde_json::Map::new();

    // Start with template values as defaults
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

    // Overlay inline step properties (inline wins)
    if let Some(obj) = inline_step.as_object() {
        for (key, value) in obj {
            // Skip null values and step_template_id itself — don't override with null
            if key == "step_template_id" {
                // Preserve the reference for audit/debugging
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
