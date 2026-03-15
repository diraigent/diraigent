use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::events::list_recent_events;
use super::integrations::list_agent_integrations;
use super::projects::get_project_by_id;
use super::roles::get_role;
use super::{Table, fetch_by_id, generate_agent_api_key};

// ── Agents ──

pub async fn register_agent(
    pool: &PgPool,
    req: &CreateAgent,
    owner_id: Uuid,
) -> Result<(Agent, String), AppError> {
    let capabilities = req.capabilities.clone().unwrap_or_default();
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let (api_key, api_key_hash) = generate_agent_api_key();

    let agent = sqlx::query_as::<_, Agent>(
        "INSERT INTO diraigent.agent (name, capabilities, metadata, owner_id, api_key_hash)
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(&req.name)
    .bind(&capabilities)
    .bind(&metadata)
    .bind(owner_id)
    .bind(&api_key_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("agent_name_key") => {
            AppError::Conflict(format!("Agent '{}' already exists", req.name))
        }
        _ => e.into(),
    })?;

    Ok((agent, api_key))
}

/// Look up an agent by API key hash. Returns (agent_id, owner_id).
pub async fn authenticate_agent_key(
    pool: &PgPool,
    key_hash: &str,
) -> Result<Option<(Uuid, Uuid)>, AppError> {
    let row: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "UPDATE diraigent.agent SET last_seen_at = now()
         WHERE api_key_hash = $1 AND status != 'revoked'
         RETURNING id, owner_id",
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|(id, owner)| owner.map(|o| (id, o))))
}

pub async fn verify_agent_owner(
    pool: &PgPool,
    agent_id: Uuid,
    user_id: Uuid,
) -> Result<bool, AppError> {
    let result: Option<Option<Uuid>> =
        sqlx::query_scalar("SELECT owner_id FROM diraigent.agent WHERE id = $1")
            .bind(agent_id)
            .fetch_optional(pool)
            .await?;

    match result {
        // Agent not found
        None => Ok(false),
        // Agent exists with no owner (legacy) — allow any authenticated user
        Some(None) => Ok(true),
        // Agent exists with an owner — must match
        Some(Some(owner)) => Ok(owner == user_id),
    }
}

pub async fn get_agent_by_id(pool: &PgPool, id: Uuid) -> Result<Agent, AppError> {
    fetch_by_id(pool, Table::Agent, id, "Agent not found").await
}

pub async fn list_agents(pool: &PgPool, p: &Pagination) -> Result<Vec<Agent>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let agents = sqlx::query_as::<_, Agent>(
        "SELECT * FROM diraigent.agent ORDER BY name ASC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(agents)
}

/// List agents visible within a tenant: agents that are members of the tenant
/// OR owned by the given user.
pub async fn list_tenant_agents(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    p: &Pagination,
) -> Result<Vec<Agent>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let agents = sqlx::query_as::<_, Agent>(
        "SELECT DISTINCT a.* FROM diraigent.agent a
         LEFT JOIN diraigent.membership m ON m.agent_id = a.id AND m.tenant_id = $1
         WHERE m.id IS NOT NULL OR a.owner_id = $2
         ORDER BY a.name ASC LIMIT $3 OFFSET $4",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(agents)
}

pub async fn update_agent(pool: &PgPool, id: Uuid, req: &UpdateAgent) -> Result<Agent, AppError> {
    let existing = get_agent_by_id(pool, id).await?;
    let name = req.name.as_deref().unwrap_or(&existing.name);
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let capabilities = req.capabilities.as_ref().unwrap_or(&existing.capabilities);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    // When revoking, also nullify the API key hash so the key can never be used again.
    let clear_key = status == "revoked" && existing.status != "revoked";

    let agent = if clear_key {
        sqlx::query_as::<_, Agent>(
            "UPDATE diraigent.agent SET name = $2, status = $3, capabilities = $4, metadata = $5, api_key_hash = NULL
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(name)
        .bind(status)
        .bind(capabilities)
        .bind(metadata)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as::<_, Agent>(
            "UPDATE diraigent.agent SET name = $2, status = $3, capabilities = $4, metadata = $5
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(name)
        .bind(status)
        .bind(capabilities)
        .bind(metadata)
        .fetch_one(pool)
        .await?
    };

    Ok(agent)
}

pub async fn agent_heartbeat(
    pool: &PgPool,
    id: Uuid,
    status: Option<&str>,
) -> Result<Agent, AppError> {
    let status = status.unwrap_or("idle");

    let agent = sqlx::query_as::<_, Agent>(
        "UPDATE diraigent.agent SET last_seen_at = now(), status = $2
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(status)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Agent not found".into()))?;

    Ok(agent)
}

pub async fn list_agent_tasks(
    pool: &PgPool,
    agent_id: Uuid,
    p: &Pagination,
) -> Result<Vec<Task>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let tasks = sqlx::query_as::<_, Task>(
        "SELECT * FROM diraigent.task WHERE assigned_agent_id = $1
         ORDER BY urgent DESC, created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(agent_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(tasks)
}

// ── Agent Context ──

pub async fn get_agent_context(
    pool: &PgPool,
    agent_id: Uuid,
    project_id: Uuid,
) -> Result<Option<AgentContext>, AppError> {
    // 1. Get agent
    let agent = get_agent_by_id(pool, agent_id).await?;

    // 2. Get membership for this agent (global — not project-scoped)
    let membership = sqlx::query_as::<_, Membership>(
        "SELECT * FROM diraigent.membership WHERE agent_id = $1 AND status = 'active' LIMIT 1",
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?;

    let membership = match membership {
        Some(m) => m,
        None => return Ok(None), // Agent has no active membership
    };

    // 3. Get role
    let role = get_role(pool, membership.role_id).await?;

    // 4. Get project
    let project = get_project_by_id(pool, project_id).await?;

    // 5. Knowledge — filtered by role's knowledge_scope
    let knowledge = if role.knowledge_scope.is_empty() {
        // No scope filter = load all
        sqlx::query_as::<_, Knowledge>(
            "SELECT * FROM diraigent.knowledge WHERE project_id = $1 ORDER BY title",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Knowledge>(
            "SELECT * FROM diraigent.knowledge WHERE project_id = $1 AND category = ANY($2) ORDER BY title"
        )
        .bind(project_id)
        .bind(&role.knowledge_scope)
        .fetch_all(pool)
        .await?
    };

    // 6. Active decisions
    let decisions = sqlx::query_as::<_, Decision>(
        "SELECT * FROM diraigent.decision WHERE project_id = $1 AND status = 'accepted' ORDER BY updated_at DESC LIMIT 50"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // 7. Available integrations (via agent_integration)
    let integrations = list_agent_integrations(pool, agent_id).await?;

    // 8. Ready tasks matching agent's capabilities
    let ready_tasks = sqlx::query_as::<_, Task>(
        "SELECT t.* FROM diraigent.task t
         WHERE t.project_id = $1 AND (t.state = 'ready' OR t.state LIKE 'wait:%')
           AND NOT EXISTS (
               SELECT 1 FROM diraigent.task_dependency td
               JOIN diraigent.task dep ON td.depends_on = dep.id
               WHERE td.task_id = t.id AND dep.state != 'done'
           )
           AND (
               t.required_capabilities = '{}'
               OR t.required_capabilities <@ $2
           )
         ORDER BY t.urgent DESC, t.created_at ASC LIMIT 20",
    )
    .bind(project_id)
    .bind(&agent.capabilities)
    .fetch_all(pool)
    .await?;

    // 9. Tasks currently assigned to this agent
    let my_tasks = sqlx::query_as::<_, Task>(
        "SELECT * FROM diraigent.task WHERE assigned_agent_id = $1 AND project_id = $2 AND state NOT IN ('done', 'cancelled') ORDER BY urgent DESC, created_at ASC"
    )
    .bind(agent_id)
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // 10. Open observations
    let open_observations = sqlx::query_as::<_, Observation>(
        "SELECT * FROM diraigent.observation WHERE project_id = $1 AND status = 'open' ORDER BY severity DESC, created_at DESC LIMIT 20"
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // 11. Recent events
    let recent_events = list_recent_events(pool, project_id, 20).await?;

    // 12. Playbooks (global)
    let playbooks =
        sqlx::query_as::<_, Playbook>("SELECT * FROM diraigent.playbook ORDER BY title")
            .fetch_all(pool)
            .await?;

    Ok(Some(AgentContext {
        agent,
        membership,
        role,
        project,
        knowledge,
        decisions,
        integrations,
        ready_tasks,
        my_tasks,
        open_observations,
        recent_events,
        playbooks,
    }))
}

// ── Wrapped Keys ──

pub async fn create_wrapped_key(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    req: &CreateWrappedKey,
) -> Result<WrappedKey, AppError> {
    let key_version = req.key_version.unwrap_or(1);
    sqlx::query_as::<_, WrappedKey>(
        "INSERT INTO diraigent.wrapped_key (tenant_id, user_id, key_type, wrapped_dek, kdf_salt, kdf_params, key_version)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(&req.key_type)
    .bind(&req.wrapped_dek)
    .bind(&req.kdf_salt)
    .bind(&req.kdf_params)
    .bind(key_version)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn list_wrapped_keys(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<WrappedKey>, AppError> {
    Ok(sqlx::query_as::<_, WrappedKey>(
        "SELECT * FROM diraigent.wrapped_key WHERE tenant_id = $1 AND user_id = $2 ORDER BY key_version DESC",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn delete_wrapped_key(pool: &PgPool, key_id: Uuid) -> Result<(), AppError> {
    super::delete_by_id(pool, Table::WrappedKey, key_id, "Wrapped key not found").await
}
