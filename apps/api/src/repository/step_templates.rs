use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, delete_by_id, fetch_by_id};

// ── Step Templates ──

pub async fn create_step_template(
    pool: &PgPool,
    tenant_id: Uuid,
    req: &CreateStepTemplate,
    created_by: Uuid,
) -> Result<StepTemplate, AppError> {
    let tags = req.tags.clone().unwrap_or_default();
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let t = sqlx::query_as::<_, StepTemplate>(
        "INSERT INTO diraigent.step_template
         (tenant_id, name, description, model, budget, allowed_tools, context_level,
          on_complete, retriable, max_cycles, timeout_minutes, mcp_servers, agents,
          agent, settings, env, vars, tags, metadata, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.model)
    .bind(req.budget)
    .bind(&req.allowed_tools)
    .bind(&req.context_level)
    .bind(&req.on_complete)
    .bind(req.retriable)
    .bind(req.max_cycles)
    .bind(req.timeout_minutes)
    .bind(&req.mcp_servers)
    .bind(&req.agents)
    .bind(&req.agent)
    .bind(&req.settings)
    .bind(&req.env)
    .bind(&req.vars)
    .bind(&tags)
    .bind(&metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(t)
}

pub async fn get_step_template_by_id(pool: &PgPool, id: Uuid) -> Result<StepTemplate, AppError> {
    fetch_by_id(pool, Table::StepTemplate, id, "Step template not found").await
}

pub async fn list_step_templates(
    pool: &PgPool,
    tenant_id: Uuid,
    filters: &StepTemplateFilters,
) -> Result<Vec<StepTemplate>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    // Return templates that belong to this tenant OR are global (tenant_id IS NULL)
    let items = sqlx::query_as::<_, StepTemplate>(
        "SELECT * FROM diraigent.step_template
         WHERE (tenant_id = $1 OR tenant_id IS NULL)
           AND ($2::text IS NULL OR name = $2)
           AND ($3::text IS NULL OR $3 = ANY(tags))
         ORDER BY created_at DESC LIMIT $4 OFFSET $5",
    )
    .bind(tenant_id)
    .bind(&filters.name)
    .bind(&filters.tag)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn update_step_template(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
    req: &UpdateStepTemplate,
) -> Result<StepTemplate, AppError> {
    let existing = get_step_template_by_id(pool, id).await?;

    // Only tenant-owned templates can be updated (not global ones)
    if existing.tenant_id.is_none() {
        return Err(AppError::Forbidden(
            "Cannot update a global step template. Fork it instead.".into(),
        ));
    }
    // Ensure the caller's tenant owns this template
    if existing.tenant_id != Some(tenant_id) {
        return Err(AppError::NotFound("Step template not found".into()));
    }

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let model = req.model.as_deref().or(existing.model.as_deref());
    let budget = req.budget.or(existing.budget);
    let allowed_tools = req
        .allowed_tools
        .as_deref()
        .or(existing.allowed_tools.as_deref());
    let context_level = req
        .context_level
        .as_deref()
        .or(existing.context_level.as_deref());
    let on_complete = req
        .on_complete
        .as_deref()
        .or(existing.on_complete.as_deref());
    let retriable = req.retriable.or(existing.retriable);
    let max_cycles = req.max_cycles.or(existing.max_cycles);
    let timeout_minutes = req.timeout_minutes.or(existing.timeout_minutes);
    let mcp_servers = req.mcp_servers.as_ref().or(existing.mcp_servers.as_ref());
    let agents = req.agents.as_ref().or(existing.agents.as_ref());
    let agent = req.agent.as_deref().or(existing.agent.as_deref());
    let settings = req.settings.as_ref().or(existing.settings.as_ref());
    let env = req.env.as_ref().or(existing.env.as_ref());
    let vars = req.vars.as_ref().or(existing.vars.as_ref());
    let tags = req.tags.as_ref().unwrap_or(&existing.tags);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let t = sqlx::query_as::<_, StepTemplate>(
        "UPDATE diraigent.step_template
         SET name = $2, description = $3, model = $4, budget = $5, allowed_tools = $6,
             context_level = $7, on_complete = $8, retriable = $9, max_cycles = $10,
             timeout_minutes = $11, mcp_servers = $12, agents = $13, agent = $14,
             settings = $15, env = $16, vars = $17, tags = $18, metadata = $19
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(model)
    .bind(budget)
    .bind(allowed_tools)
    .bind(context_level)
    .bind(on_complete)
    .bind(retriable)
    .bind(max_cycles)
    .bind(timeout_minutes)
    .bind(mcp_servers)
    .bind(agents)
    .bind(agent)
    .bind(settings)
    .bind(env)
    .bind(vars)
    .bind(tags)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(t)
}

pub async fn delete_step_template(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
) -> Result<(), AppError> {
    let existing = get_step_template_by_id(pool, id).await?;

    // Only tenant-owned templates can be deleted (not global ones)
    if existing.tenant_id.is_none() {
        return Err(AppError::Forbidden(
            "Cannot delete a global step template.".into(),
        ));
    }
    // Ensure the caller's tenant owns this template
    if existing.tenant_id != Some(tenant_id) {
        return Err(AppError::NotFound("Step template not found".into()));
    }

    delete_by_id(pool, Table::StepTemplate, id, "Step template not found").await
}

/// Fork a global (tenant_id = NULL) or any template into a tenant-owned copy,
/// applying any fields from `req` on top of the source.
pub async fn fork_step_template(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
    req: &UpdateStepTemplate,
    created_by: Uuid,
) -> Result<StepTemplate, AppError> {
    let source = get_step_template_by_id(pool, id).await?;

    let name = req.name.as_deref().unwrap_or(&source.name);
    let description = req.description.as_deref().or(source.description.as_deref());
    let model = req.model.as_deref().or(source.model.as_deref());
    let budget = req.budget.or(source.budget);
    let allowed_tools = req
        .allowed_tools
        .as_deref()
        .or(source.allowed_tools.as_deref());
    let context_level = req
        .context_level
        .as_deref()
        .or(source.context_level.as_deref());
    let on_complete = req.on_complete.as_deref().or(source.on_complete.as_deref());
    let retriable = req.retriable.or(source.retriable);
    let max_cycles = req.max_cycles.or(source.max_cycles);
    let timeout_minutes = req.timeout_minutes.or(source.timeout_minutes);
    let mcp_servers = req.mcp_servers.as_ref().or(source.mcp_servers.as_ref());
    let agents = req.agents.as_ref().or(source.agents.as_ref());
    let agent = req.agent.as_deref().or(source.agent.as_deref());
    let settings = req.settings.as_ref().or(source.settings.as_ref());
    let env = req.env.as_ref().or(source.env.as_ref());
    let vars = req.vars.as_ref().or(source.vars.as_ref());
    let tags = req.tags.as_ref().unwrap_or(&source.tags);
    let metadata = req.metadata.as_ref().unwrap_or(&source.metadata);

    let t = sqlx::query_as::<_, StepTemplate>(
        "INSERT INTO diraigent.step_template
         (tenant_id, name, description, model, budget, allowed_tools, context_level,
          on_complete, retriable, max_cycles, timeout_minutes, mcp_servers, agents,
          agent, settings, env, vars, tags, metadata, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(description)
    .bind(model)
    .bind(budget)
    .bind(allowed_tools)
    .bind(context_level)
    .bind(on_complete)
    .bind(retriable)
    .bind(max_cycles)
    .bind(timeout_minutes)
    .bind(mcp_servers)
    .bind(agents)
    .bind(agent)
    .bind(settings)
    .bind(env)
    .bind(vars)
    .bind(tags)
    .bind(metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(t)
}
