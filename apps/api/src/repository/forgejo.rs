use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CiJob, CiRun, CiStep, ForgejoIntegration};

/// Look up the Forgejo integration for a given project.
pub async fn get_forgejo_integration_by_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<ForgejoIntegration, AppError> {
    sqlx::query_as::<_, ForgejoIntegration>(
        "SELECT * FROM diraigent.forgejo_integration WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Forgejo integration not found".into()))
}

/// Look up the Forgejo integration by its own ID (used for webhook routing).
pub async fn get_forgejo_integration(
    pool: &PgPool,
    id: Uuid,
) -> Result<ForgejoIntegration, AppError> {
    sqlx::query_as::<_, ForgejoIntegration>(
        "SELECT * FROM diraigent.forgejo_integration WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Forgejo integration not found".into()))
}

/// Upsert a CI run record from a webhook event.
/// Uses ON CONFLICT to update an existing run if the (project_id, forgejo_run_id) pair
/// already exists.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_ci_run(
    pool: &PgPool,
    project_id: Uuid,
    forgejo_run_id: i64,
    workflow_name: &str,
    status: &str,
    branch: Option<&str>,
    commit_sha: Option<&str>,
    triggered_by: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
) -> Result<CiRun, AppError> {
    let row = sqlx::query_as::<_, CiRun>(
        "INSERT INTO diraigent.ci_run (project_id, forgejo_run_id, workflow_name, status, branch, commit_sha, triggered_by, started_at, finished_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (project_id, forgejo_run_id)
         DO UPDATE SET status = EXCLUDED.status,
                       workflow_name = EXCLUDED.workflow_name,
                       branch = EXCLUDED.branch,
                       commit_sha = EXCLUDED.commit_sha,
                       triggered_by = EXCLUDED.triggered_by,
                       started_at = EXCLUDED.started_at,
                       finished_at = EXCLUDED.finished_at
         RETURNING *",
    )
    .bind(project_id)
    .bind(forgejo_run_id)
    .bind(workflow_name)
    .bind(status)
    .bind(branch)
    .bind(commit_sha)
    .bind(triggered_by)
    .bind(started_at)
    .bind(finished_at)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Look up a CI run by the project's internal ID and the Forgejo run ID.
pub async fn get_ci_run_by_forgejo_id(
    pool: &PgPool,
    project_id: Uuid,
    forgejo_run_id: i64,
) -> Result<Option<CiRun>, AppError> {
    let row = sqlx::query_as::<_, CiRun>(
        "SELECT * FROM diraigent.ci_run WHERE project_id = $1 AND forgejo_run_id = $2",
    )
    .bind(project_id)
    .bind(forgejo_run_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Upsert a CI job by (ci_run_id, name).
///
/// If a job with the same name already exists under the given run, it is updated
/// in-place (preserving its UUID). Otherwise a new row is inserted.
/// Uses a writable CTE for atomicity.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_ci_job_by_name(
    pool: &PgPool,
    ci_run_id: Uuid,
    name: &str,
    status: &str,
    runner: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
) -> Result<CiJob, AppError> {
    let row = sqlx::query_as::<_, CiJob>(
        "WITH existing AS (
             SELECT id FROM diraigent.ci_job
             WHERE ci_run_id = $1 AND name = $2
             LIMIT 1
         ),
         updated AS (
             UPDATE diraigent.ci_job
             SET status = $3, runner = $4, started_at = $5, finished_at = $6
             WHERE id = (SELECT id FROM existing)
             RETURNING *
         ),
         inserted AS (
             INSERT INTO diraigent.ci_job (ci_run_id, name, status, runner, started_at, finished_at)
             SELECT $1, $2, $3, $4, $5, $6
             WHERE NOT EXISTS (SELECT 1 FROM existing)
             RETURNING *
         )
         SELECT * FROM updated
         UNION ALL
         SELECT * FROM inserted",
    )
    .bind(ci_run_id)
    .bind(name)
    .bind(status)
    .bind(runner)
    .bind(started_at)
    .bind(finished_at)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Delete all steps for a given job (before re-inserting from the API).
pub async fn delete_steps_for_job(pool: &PgPool, ci_job_id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM diraigent.ci_step WHERE ci_job_id = $1")
        .bind(ci_job_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Insert a single CI step record.
#[allow(clippy::too_many_arguments)]
pub async fn insert_ci_step(
    pool: &PgPool,
    ci_job_id: Uuid,
    name: &str,
    status: &str,
    exit_code: Option<i32>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
) -> Result<CiStep, AppError> {
    let row = sqlx::query_as::<_, CiStep>(
        "INSERT INTO diraigent.ci_step (ci_job_id, name, status, exit_code, started_at, finished_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(ci_job_id)
    .bind(name)
    .bind(status)
    .bind(exit_code)
    .bind(started_at)
    .bind(finished_at)
    .fetch_one(pool)
    .await?;

    Ok(row)
}
