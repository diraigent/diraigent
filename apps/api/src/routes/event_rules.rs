use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;
use crate::models::*;
use crate::repository;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/{project_id}/event-rules",
            post(create_event_rule).get(list_event_rules),
        )
        .route(
            "/event-rules/{id}",
            get(get_event_rule)
                .put(update_event_rule)
                .delete(delete_event_rule),
        )
        .route("/event-rules/{id}/toggle", post(toggle_event_rule))
}

async fn create_event_rule(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateEventObservationRule>,
) -> Result<Json<EventObservationRule>, AppError> {
    validation::validate_create_event_observation_rule(&req)?;
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let rule = repository::create_event_observation_rule(&state.pool, project_id, &req).await?;

    state.fire_event(
        project_id,
        "event_rule.created",
        "event_observation_rule",
        rule.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"rule_id": rule.id, "name": rule.name}),
    );

    Ok(Json(rule))
}

async fn list_event_rules(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<EventObservationRuleFilters>,
) -> Result<Json<Vec<EventObservationRule>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let rules = repository::list_event_observation_rules(&state.pool, project_id, &filters).await?;
    Ok(Json(rules))
}

async fn get_event_rule(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<EventObservationRule>, AppError> {
    let rule = repository::get_event_observation_rule(&state.pool, id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, rule.project_id).await?;
    Ok(Json(rule))
}

async fn update_event_rule(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateEventObservationRule>,
) -> Result<Json<EventObservationRule>, AppError> {
    let existing = repository::get_event_observation_rule(&state.pool, id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, existing.project_id).await?;
    // Validate fields if present
    if let Some(ref name) = req.name
        && name.is_empty()
    {
        return Err(AppError::Validation("Rule name must be non-empty".into()));
    }
    if let Some(ref kind) = req.observation_kind
        && !OBSERVATION_KINDS.contains(&kind.as_str())
    {
        return Err(AppError::Validation(format!(
            "Invalid observation kind: {}. Valid: {:?}",
            kind, OBSERVATION_KINDS
        )));
    }
    if let Some(ref sev) = req.observation_severity
        && !OBSERVATION_SEVERITIES.contains(&sev.as_str())
    {
        return Err(AppError::Validation(format!(
            "Invalid observation severity: {}. Valid: {:?}",
            sev, OBSERVATION_SEVERITIES
        )));
    }
    if let Some(ref ek) = req.event_kind
        && !EVENT_KINDS.contains(&ek.as_str())
    {
        return Err(AppError::Validation(format!(
            "Invalid event kind: {}. Valid: {:?}",
            ek, EVENT_KINDS
        )));
    }
    if let Some(ref sev) = req.severity_gte
        && !EVENT_SEVERITIES.contains(&sev.as_str())
    {
        return Err(AppError::Validation(format!(
            "Invalid severity_gte: {}. Valid: {:?}",
            sev, EVENT_SEVERITIES
        )));
    }
    if let Some(ref tt) = req.title_template
        && tt.is_empty()
    {
        return Err(AppError::Validation(
            "title_template must be non-empty".into(),
        ));
    }
    let rule = repository::update_event_observation_rule(&state.pool, id, &req).await?;
    Ok(Json(rule))
}

async fn delete_event_rule(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let existing = repository::get_event_observation_rule(&state.pool, id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, existing.project_id).await?;
    repository::delete_event_observation_rule(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn toggle_event_rule(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<EventObservationRule>, AppError> {
    let existing = repository::get_event_observation_rule(&state.pool, id).await?;
    require_membership(state.db.as_ref(), agent_id, user_id, existing.project_id).await?;
    let update = UpdateEventObservationRule {
        enabled: Some(!existing.enabled),
        name: None,
        event_kind: None,
        event_source: None,
        severity_gte: None,
        observation_kind: None,
        observation_severity: None,
        title_template: None,
        description_template: None,
    };
    let rule = repository::update_event_observation_rule(&state.pool, id, &update).await?;
    Ok(Json(rule))
}
