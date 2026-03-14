use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::Table;
use super::delete_by_id;
use super::fetch_by_id;

const EVENT_RULE_FILTERS_WHERE: &str = "WHERE project_id = $1 \
    AND ($2::bool IS NULL OR enabled = $2)";

// ── Event Observation Rules ──

pub async fn create_event_observation_rule(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateEventObservationRule,
) -> Result<EventObservationRule, AppError> {
    let rule = sqlx::query_as::<_, EventObservationRule>(
        "INSERT INTO diraigent.event_observation_rule \
             (project_id, name, enabled, event_kind, event_source, severity_gte, \
              observation_kind, observation_severity, title_template, description_template) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *",
    )
    .bind(project_id)
    .bind(&req.name)
    .bind(req.enabled.unwrap_or(true))
    .bind(&req.event_kind)
    .bind(&req.event_source)
    .bind(&req.severity_gte)
    .bind(req.observation_kind.as_deref().unwrap_or("insight"))
    .bind(req.observation_severity.as_deref().unwrap_or("info"))
    .bind(&req.title_template)
    .bind(&req.description_template)
    .fetch_one(pool)
    .await?;

    Ok(rule)
}

pub async fn get_event_observation_rule(
    pool: &PgPool,
    id: Uuid,
) -> Result<EventObservationRule, AppError> {
    fetch_by_id(
        pool,
        Table::EventObservationRule,
        id,
        "Event observation rule not found",
    )
    .await
}

pub async fn list_event_observation_rules(
    pool: &PgPool,
    project_id: Uuid,
    filters: &EventObservationRuleFilters,
) -> Result<Vec<EventObservationRule>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let sql = format!(
        "SELECT * FROM diraigent.event_observation_rule {} ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        EVENT_RULE_FILTERS_WHERE
    );
    let items = sqlx::query_as::<_, EventObservationRule>(&sql)
        .bind(project_id)
        .bind(filters.enabled)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(items)
}

pub async fn update_event_observation_rule(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateEventObservationRule,
) -> Result<EventObservationRule, AppError> {
    let existing = get_event_observation_rule(pool, id).await?;

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let enabled = req.enabled.unwrap_or(existing.enabled);
    let event_kind = req.event_kind.as_deref().or(existing.event_kind.as_deref());
    let event_source = req
        .event_source
        .as_deref()
        .or(existing.event_source.as_deref());
    let severity_gte = req
        .severity_gte
        .as_deref()
        .or(existing.severity_gte.as_deref());
    let observation_kind = req
        .observation_kind
        .as_deref()
        .unwrap_or(&existing.observation_kind);
    let observation_severity = req
        .observation_severity
        .as_deref()
        .unwrap_or(&existing.observation_severity);
    let title_template = req
        .title_template
        .as_deref()
        .unwrap_or(&existing.title_template);
    let description_template = req
        .description_template
        .as_deref()
        .or(existing.description_template.as_deref());

    let rule = sqlx::query_as::<_, EventObservationRule>(
        "UPDATE diraigent.event_observation_rule \
         SET name = $2, enabled = $3, event_kind = $4, event_source = $5, \
             severity_gte = $6, observation_kind = $7, observation_severity = $8, \
             title_template = $9, description_template = $10, updated_at = now() \
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(enabled)
    .bind(event_kind)
    .bind(event_source)
    .bind(severity_gte)
    .bind(observation_kind)
    .bind(observation_severity)
    .bind(title_template)
    .bind(description_template)
    .fetch_one(pool)
    .await?;

    Ok(rule)
}

pub async fn delete_event_observation_rule(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(
        pool,
        Table::EventObservationRule,
        id,
        "Event observation rule not found",
    )
    .await
}

/// Find enabled rules matching the given event criteria.
///
/// A rule matches when:
/// - `enabled` is true
/// - `event_kind` is NULL (match any) or equals the given kind
/// - `event_source` is NULL (match any) or equals the given source
/// - `severity_gte` is NULL (match any) or the event severity meets the threshold
///
/// Severity ordering: info < warning < error < critical
pub async fn find_matching_rules(
    pool: &PgPool,
    project_id: Uuid,
    event_kind: &str,
    event_source: &str,
    event_severity: Option<&str>,
) -> Result<Vec<EventObservationRule>, AppError> {
    // Map severity strings to numeric levels for comparison.
    // If the event has no severity, only rules without severity_gte will match.
    let severity_level = event_severity.map(severity_to_level).unwrap_or(0);

    let rules = sqlx::query_as::<_, EventObservationRule>(
        "SELECT * FROM diraigent.event_observation_rule \
         WHERE project_id = $1 \
           AND enabled = true \
           AND (event_kind IS NULL OR event_kind = $2) \
           AND (event_source IS NULL OR event_source = $3) \
           AND (severity_gte IS NULL OR CASE severity_gte \
               WHEN 'info' THEN 0 \
               WHEN 'warning' THEN 1 \
               WHEN 'error' THEN 2 \
               WHEN 'critical' THEN 3 \
               ELSE 0 END <= $4) \
         ORDER BY created_at ASC",
    )
    .bind(project_id)
    .bind(event_kind)
    .bind(event_source)
    .bind(severity_level)
    .fetch_all(pool)
    .await?;

    Ok(rules)
}

/// Map severity string to a numeric level for comparison.
fn severity_to_level(s: &str) -> i32 {
    match s {
        "info" => 0,
        "warning" => 1,
        "error" => 2,
        "critical" => 3,
        _ => 0,
    }
}
