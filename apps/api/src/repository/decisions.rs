use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::projects::get_project_by_id;
use super::{Table, delete_by_id, fetch_by_id};

const DECISION_FILTERS_WHERE: &str = "WHERE project_id = $1 AND ($2::text IS NULL OR status = $2)";

// ── Decisions ──

pub async fn create_decision(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateDecision,
    created_by: Uuid,
) -> Result<Decision, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let alternatives = sqlx::types::Json(req.alternatives.clone().unwrap_or_default());
    let tags = req.tags.clone().unwrap_or_default();

    let d = sqlx::query_as::<_, Decision>(
        "INSERT INTO diraigent.decision (project_id, title, context, decision, rationale, alternatives, consequences, tags, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(&req.context)
    .bind(&req.decision)
    .bind(&req.rationale)
    .bind(&alternatives)
    .bind(&req.consequences)
    .bind(&tags)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(d)
}

pub async fn get_decision_by_id(pool: &PgPool, id: Uuid) -> Result<Decision, AppError> {
    fetch_by_id(pool, Table::Decision, id, "Decision not found").await
}

super::list_and_count!(
    list_decisions,
    count_decisions,
    Decision,
    DecisionFilters,
    "decision",
    DECISION_FILTERS_WHERE,
    |f| f.limit,
    |f| f.offset,
    |q, f| q.bind(&f.status)
);

pub async fn update_decision(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateDecision,
) -> Result<Decision, AppError> {
    let existing = get_decision_by_id(pool, id).await?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let context = req.context.as_deref().unwrap_or(&existing.context);
    let decision = req.decision.as_deref().or(existing.decision.as_deref());
    let rationale = req.rationale.as_deref().or(existing.rationale.as_deref());
    let alternatives_owned: Vec<_>;
    let alternatives = if let Some(a) = req.alternatives.as_ref() {
        a
    } else {
        alternatives_owned = existing.alternatives.clone();
        &alternatives_owned
    };
    let alternatives = sqlx::types::Json(alternatives);
    let consequences = req
        .consequences
        .as_deref()
        .or(existing.consequences.as_deref());
    let superseded_by = req.superseded_by.or(existing.superseded_by);
    let decided_by = req.decided_by.or(existing.decided_by);
    let tags = req.tags.as_ref().unwrap_or(&existing.tags);

    let d = sqlx::query_as::<_, Decision>(
        "UPDATE diraigent.decision
         SET title = $2, status = $3, context = $4, decision = $5, rationale = $6,
             alternatives = $7, consequences = $8, superseded_by = $9, decided_by = $10, tags = $11
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(status)
    .bind(context)
    .bind(decision)
    .bind(rationale)
    .bind(alternatives)
    .bind(consequences)
    .bind(superseded_by)
    .bind(decided_by)
    .bind(tags)
    .fetch_one(pool)
    .await?;

    Ok(d)
}

pub async fn delete_decision(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Decision, id, "Decision not found").await
}
