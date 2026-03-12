use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AgentSseEvent;
use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::*;
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
    Json(req): Json<CreateAgent>,
) -> Result<Json<AgentRegistered>, AppError> {
    validation::validate_create_agent(&req)?;
    let (agent, api_key) = state.db.register_agent(&req, user_id).await?;
    Ok(Json(AgentRegistered { agent, api_key }))
}

async fn list_agents(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Agent>>, AppError> {
    let agents = state.db.list_agents(&pagination).await?;
    Ok(Json(agents))
}

async fn get_agent(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Agent>, AppError> {
    let agent = state.db.get_agent_by_id(agent_id).await?;
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
    AuthUser(_): AuthUser,
    Path(agent_id): Path<Uuid>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<Vec<Task>>, AppError> {
    let tasks = state.db.list_agent_tasks(agent_id, &pagination).await?;
    Ok(Json(tasks))
}
