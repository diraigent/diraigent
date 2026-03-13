use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::Table;
use super::fetch_by_id;
use super::playbooks::get_playbook_by_id;
use super::projects::get_project_by_id;

const OBSERVATION_FILTERS_WHERE: &str = "WHERE project_id = $1 \
    AND ($2::text IS NULL OR kind = $2) \
    AND ($3::text IS NULL OR severity = $3) \
    AND ($4::text IS NULL OR status = $4)";

// ── Observations ──

pub async fn create_observation(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateObservation,
) -> Result<Observation, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let kind = req.kind.as_deref().unwrap_or("insight");
    let severity = req.severity.as_deref().unwrap_or("low");
    let evidence = req.evidence.clone().unwrap_or(serde_json::json!({}));
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let o = sqlx::query_as::<_, Observation>(
        "INSERT INTO diraigent.observation (project_id, agent_id, kind, title, description, severity, source, source_task_id, evidence, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING *",
    )
    .bind(project_id)
    .bind(req.agent_id)
    .bind(kind)
    .bind(&req.title)
    .bind(&req.description)
    .bind(severity)
    .bind(&req.source)
    .bind(req.source_task_id)
    .bind(&evidence)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(o)
}

pub async fn get_observation_by_id(pool: &PgPool, id: Uuid) -> Result<Observation, AppError> {
    fetch_by_id(pool, Table::Observation, id, "Observation not found").await
}

pub async fn list_observations(
    pool: &PgPool,
    project_id: Uuid,
    filters: &ObservationFilters,
) -> Result<Vec<Observation>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let sql = format!(
        "SELECT * FROM diraigent.observation {} ORDER BY created_at DESC LIMIT $5 OFFSET $6",
        OBSERVATION_FILTERS_WHERE
    );
    let items = sqlx::query_as::<_, Observation>(&sql)
        .bind(project_id)
        .bind(&filters.kind)
        .bind(&filters.severity)
        .bind(&filters.status)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(items)
}

pub async fn update_observation(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateObservation,
) -> Result<Observation, AppError> {
    let existing = get_observation_by_id(pool, id).await?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let severity = req.severity.as_deref().unwrap_or(&existing.severity);
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let evidence = req.evidence.as_ref().unwrap_or(&existing.evidence);
    let resolved_task_id = req.resolved_task_id.or(existing.resolved_task_id);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let o = sqlx::query_as::<_, Observation>(
        "UPDATE diraigent.observation
         SET title = $2, description = $3, severity = $4, status = $5, evidence = $6, resolved_task_id = $7, metadata = $8
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(description)
    .bind(severity)
    .bind(status)
    .bind(evidence)
    .bind(resolved_task_id)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(o)
}

pub async fn dismiss_observation(pool: &PgPool, id: Uuid) -> Result<Observation, AppError> {
    let o = sqlx::query_as::<_, Observation>(
        "UPDATE diraigent.observation SET status = 'dismissed' WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Observation not found".into()))?;
    Ok(o)
}

pub async fn promote_observation(
    pool: &PgPool,
    id: Uuid,
    req: &PromoteObservation,
    created_by: Uuid,
) -> Result<(Observation, Task), AppError> {
    let obs = get_observation_by_id(pool, id).await?;
    if obs.status == "acted_on" {
        return Err(AppError::Conflict("Observation already promoted".into()));
    }

    let task_title = req.title.clone().unwrap_or_else(|| obs.title.clone());
    let kind = req.kind.clone().unwrap_or_else(|| "chore".to_string());
    let priority = req.priority.unwrap_or(0);

    // Resolve project defaults before starting the transaction (read-only).
    let project = get_project_by_id(pool, obs.project_id).await?;
    let playbook_id = req.playbook_id.or(project.default_playbook_id);
    let initial_state = if let Some(pb_id) = playbook_id {
        let playbook = get_playbook_by_id(pool, pb_id).await?;
        playbook.initial_state.clone()
    } else {
        "backlog".to_string()
    };
    let context = serde_json::Value::Object(Default::default());
    let capabilities: Vec<String> = vec![];

    // Wrap both writes in a transaction so they succeed or fail atomically.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO diraigent.task
             (project_id, title, kind, state, priority, context, required_capabilities,
              playbook_id, playbook_step, decision_id, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING *",
    )
    .bind(obs.project_id)
    .bind(&task_title)
    .bind(&kind)
    .bind(&initial_state)
    .bind(priority)
    .bind(&context)
    .bind(&capabilities)
    .bind(playbook_id)
    .bind(if playbook_id.is_some() {
        Some(0i32)
    } else {
        None
    })
    .bind(Option::<Uuid>::None) // decision_id
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await?;

    let updated = sqlx::query_as::<_, Observation>(
        "UPDATE diraigent.observation SET status = 'acted_on', resolved_task_id = $2 WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(task.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((updated, task))
}

pub async fn delete_observation(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    super::delete_by_id(pool, Table::Observation, id, "Observation not found").await
}

/// Default number of days to retain observations when no project-level setting is configured.
const DEFAULT_OBSERVATION_RETENTION_DAYS: i32 = 30;

pub async fn cleanup_observations(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<CleanupObservationsResult, AppError> {
    let project = get_project_by_id(pool, project_id).await?;

    // Read retention days from project metadata, falling back to the default.
    let retention_days = project
        .metadata
        .get("observation_retention_days")
        .and_then(|v| v.as_i64())
        .map(|d| d as i32)
        .unwrap_or(DEFAULT_OBSERVATION_RETENTION_DAYS);

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // 1. Delete dismissed observations
    let r1 = sqlx::query(
        "DELETE FROM diraigent.observation WHERE project_id = $1 AND status = 'dismissed'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    // 2. Delete acknowledged observations
    let r2 = sqlx::query(
        "DELETE FROM diraigent.observation WHERE project_id = $1 AND status = 'acknowledged'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    // 3. Delete acted_on observations
    let r3 = sqlx::query(
        "DELETE FROM diraigent.observation WHERE project_id = $1 AND status = 'acted_on'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    // 4. Delete open observations that have a resolved_task_id (done but still marked open)
    let r4 = sqlx::query(
        "DELETE FROM diraigent.observation \
         WHERE project_id = $1 AND resolved_task_id IS NOT NULL \
         AND status = 'open'",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    // 5. Delete duplicate observations (keep newest per title)
    let r5 = sqlx::query(
        "WITH ranked AS ( \
             SELECT id, ROW_NUMBER() OVER (PARTITION BY title ORDER BY created_at DESC) AS rn \
             FROM diraigent.observation \
             WHERE project_id = $1 \
         ) \
         DELETE FROM diraigent.observation \
         WHERE id IN (SELECT id FROM ranked WHERE rn > 1)",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    // 6. Delete observations older than the retention period
    let r6 = sqlx::query(
        "DELETE FROM diraigent.observation \
         WHERE project_id = $1 \
         AND created_at < NOW() - make_interval(days => $2)",
    )
    .bind(project_id)
    .bind(retention_days)
    .execute(&mut *tx)
    .await?;

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(CleanupObservationsResult {
        deleted_dismissed: r1.rows_affected() as i64,
        deleted_acknowledged: r2.rows_affected() as i64,
        deleted_acted_on: r3.rows_affected() as i64,
        deleted_resolved: r4.rows_affected() as i64,
        deleted_duplicates: r5.rows_affected() as i64,
        deleted_old: r6.rows_affected() as i64,
        total_deleted: (r1.rows_affected()
            + r2.rows_affected()
            + r3.rows_affected()
            + r4.rows_affected()
            + r5.rows_affected()
            + r6.rows_affected()) as i64,
    })
}

/// Delete old observations across all projects in a single efficient query.
///
/// Each project's `metadata.observation_retention_days` controls how many days
/// to keep; the global default is used when the key is absent.
///
/// Returns the total number of deleted rows.
pub async fn delete_old_observations_all_projects(
    pool: &PgPool,
    default_retention_days: i32,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        "DELETE FROM diraigent.observation o \
         USING diraigent.project p \
         WHERE o.project_id = p.id \
         AND o.created_at < NOW() - make_interval(days => \
             COALESCE((p.metadata->>'observation_retention_days')::integer, $1))",
    )
    .bind(default_retention_days)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn count_observations(
    pool: &PgPool,
    project_id: Uuid,
    filters: &ObservationFilters,
) -> Result<i64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) FROM diraigent.observation {}",
        OBSERVATION_FILTERS_WHERE
    );
    let row: (i64,) = sqlx::query_as(&sql)
        .bind(project_id)
        .bind(&filters.kind)
        .bind(&filters.severity)
        .bind(&filters.status)
        .fetch_one(pool)
        .await?;

    Ok(row.0)
}
