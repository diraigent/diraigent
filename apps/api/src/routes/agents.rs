use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AgentSseEvent;
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::*;
use crate::tenant::TenantContext;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/agents", post(register_agent).get(list_agents))
        .route("/agents/{agent_id}", get(get_agent).put(update_agent))
        .route("/agents/{agent_id}/heartbeat", post(heartbeat))
        .route("/agents/{agent_id}/tasks", get(list_agent_tasks))
}

async fn register_agent(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Json(req): Json<CreateAgent>,
) -> Result<Json<AgentRegistered>, AppError> {
    validation::validate_create_agent(&req)?;
    let (agent, api_key) = state.db.register_agent(&req, user_id).await?;

    // Auto-setup: ensure a "main" role exists and assign the new agent to it.
    // If the tenant has no roles yet, create one with all authorities.
    let roles = state.db.list_roles(tenant.tenant_id).await?;
    let role_id = if roles.is_empty() {
        let role = state
            .db
            .create_role(
                tenant.tenant_id,
                &CreateRole {
                    name: "main".into(),
                    description: Some("Default role with full access".into()),
                    authorities: Some(AUTHORITIES.iter().map(|a| a.to_string()).collect()),
                    required_capabilities: None,
                    knowledge_scope: None,
                    metadata: None,
                },
            )
            .await?;
        role.id
    } else {
        roles[0].id
    };

    // Best-effort membership creation (ignore conflicts if already assigned)
    let _ = state
        .db
        .create_membership(
            tenant.tenant_id,
            &CreateMembership {
                agent_id: agent.id,
                role_id,
                config: None,
            },
        )
        .await;

    Ok(Json(AgentRegistered { agent, api_key }))
}

async fn list_agents(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Agent>>, AppError> {
    let agents = state
        .db
        .list_tenant_agents(tenant.tenant_id, user_id, &pagination)
        .await?;
    Ok(Json(agents))
}

async fn get_agent(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Agent>, AppError> {
    let agent = state.db.get_agent_by_id(agent_id).await?;
    // Allow access if user owns the agent or the agent is a member of their tenant
    let is_owner = agent.owner_id == Some(user_id);
    if !is_owner {
        let tenant_agents = state.db.list_tenant_agent_ids(tenant.tenant_id).await?;
        if !tenant_agents.contains(&agent_id) {
            return Err(AppError::Forbidden(
                "Agent is not visible in your tenant".into(),
            ));
        }
    }
    Ok(Json(agent))
}

async fn update_agent(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<UpdateAgent>,
) -> Result<Json<Agent>, AppError> {
    let is_owner = state
        .db
        .verify_agent_owner(agent_id, user_id)
        .await
        .map_err(|_| AppError::Unauthorized("Agent ownership check failed".into()))?;
    if !is_owner {
        return Err(AppError::Forbidden("You do not own this agent".into()));
    }
    validation::validate_update_agent(&req)?;
    let agent = state.db.update_agent(agent_id, &req).await?;
    // Notify SSE subscribers if the status changed.
    if req.status.is_some() {
        let _ = state.agent_tx.send(AgentSseEvent {
            agent_id: agent.id,
            name: agent.name.clone(),
            status: agent.status.clone(),
        });
    }
    Ok(Json(agent))
}

async fn heartbeat(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<Agent>, AppError> {
    let is_owner = state
        .db
        .verify_agent_owner(agent_id, user_id)
        .await
        .map_err(|_| AppError::Unauthorized("Agent ownership check failed".into()))?;
    if !is_owner {
        return Err(AppError::Forbidden("You do not own this agent".into()));
    }
    let agent = state
        .db
        .agent_heartbeat(agent_id, req.status.as_deref())
        .await?;
    // Notify SSE subscribers of the updated status.
    let _ = state.agent_tx.send(AgentSseEvent {
        agent_id: agent.id,
        name: agent.name.clone(),
        status: agent.status.clone(),
    });
    Ok(Json(agent))
}

async fn list_agent_tasks(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Path(agent_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Task>>, AppError> {
    // Verify the agent is visible to the caller (owner or same tenant)
    let is_owner = state
        .db
        .verify_agent_owner(agent_id, user_id)
        .await
        .unwrap_or(false);
    if !is_owner {
        let tenant_agents = state.db.list_tenant_agent_ids(tenant.tenant_id).await?;
        if !tenant_agents.contains(&agent_id) {
            return Err(AppError::Forbidden(
                "Agent is not visible in your tenant".into(),
            ));
        }
    }
    let tasks = state.db.list_agent_tasks(agent_id, &pagination).await?;
    Ok(Json(tasks))
}
