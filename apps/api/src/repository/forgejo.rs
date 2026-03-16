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
/// Uses ON CONFLICT to update an existing run if the (project_id, provider, external_id)
/// triple already exists.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_ci_run(
    pool: &PgPool,
    project_id: Uuid,
    external_id: i64,
    workflow_name: &str,
    status: &str,
    branch: Option<&str>,
    commit_sha: Option<&str>,
    triggered_by: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    provider: &str,
) -> Result<CiRun, AppError> {
    let row = sqlx::query_as::<_, CiRun>(
        "INSERT INTO diraigent.ci_run (project_id, external_id, workflow_name, status, branch, commit_sha, triggered_by, started_at, finished_at, provider)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (project_id, provider, external_id)
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
    .bind(external_id)
    .bind(workflow_name)
    .bind(status)
    .bind(branch)
    .bind(commit_sha)
    .bind(triggered_by)
    .bind(started_at)
    .bind(finished_at)
    .bind(provider)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Look up a CI run by (project_id, provider, external_id).
pub async fn get_ci_run_by_external_id(
    pool: &PgPool,
    project_id: Uuid,
    provider: &str,
    external_id: i64,
) -> Result<Option<CiRun>, AppError> {
    let row = sqlx::query_as::<_, CiRun>(
        "SELECT * FROM diraigent.ci_run WHERE project_id = $1 AND provider = $2 AND external_id = $3",
    )
    .bind(project_id)
    .bind(provider)
    .bind(external_id)
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

// ── REST API query functions ──

/// List CI runs for a project with optional filters, ordered by started_at desc.
#[allow(clippy::too_many_arguments)]
pub async fn list_ci_runs(
    pool: &PgPool,
    project_id: Uuid,
    branch: Option<&str>,
    status: Option<&str>,
    workflow_name: Option<&str>,
    provider: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<CiRun>, AppError> {
    let rows = sqlx::query_as::<_, CiRun>(
        "SELECT * FROM diraigent.ci_run
         WHERE project_id = $1
           AND ($2::text IS NULL OR branch = $2)
           AND ($3::text IS NULL OR status = $3)
           AND ($4::text IS NULL OR workflow_name = $4)
           AND ($5::text IS NULL OR provider = $5)
         ORDER BY started_at DESC NULLS LAST, created_at DESC
         LIMIT $6 OFFSET $7",
    )
    .bind(project_id)
    .bind(branch)
    .bind(status)
    .bind(workflow_name)
    .bind(provider)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Count CI runs for a project with optional filters.
pub async fn count_ci_runs(
    pool: &PgPool,
    project_id: Uuid,
    branch: Option<&str>,
    status: Option<&str>,
    workflow_name: Option<&str>,
    provider: Option<&str>,
) -> Result<i64, AppError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM diraigent.ci_run
         WHERE project_id = $1
           AND ($2::text IS NULL OR branch = $2)
           AND ($3::text IS NULL OR status = $3)
           AND ($4::text IS NULL OR workflow_name = $4)
           AND ($5::text IS NULL OR provider = $5)",
    )
    .bind(project_id)
    .bind(branch)
    .bind(status)
    .bind(workflow_name)
    .bind(provider)
    .fetch_one(pool)
    .await?;

    Ok(count)
}

/// Get a single CI run by its UUID.
pub async fn get_ci_run(pool: &PgPool, id: Uuid) -> Result<CiRun, AppError> {
    sqlx::query_as::<_, CiRun>("SELECT * FROM diraigent.ci_run WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("CI run not found".into()))
}

/// List all jobs for a given CI run, ordered by started_at.
pub async fn list_ci_jobs_for_run(pool: &PgPool, ci_run_id: Uuid) -> Result<Vec<CiJob>, AppError> {
    let rows = sqlx::query_as::<_, CiJob>(
        "SELECT * FROM diraigent.ci_job WHERE ci_run_id = $1 ORDER BY started_at ASC NULLS LAST",
    )
    .bind(ci_run_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get a single CI job by its UUID.
pub async fn get_ci_job(pool: &PgPool, id: Uuid) -> Result<CiJob, AppError> {
    sqlx::query_as::<_, CiJob>("SELECT * FROM diraigent.ci_job WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("CI job not found".into()))
}

/// List all steps for a given CI job, ordered by started_at.
pub async fn list_ci_steps_for_job(
    pool: &PgPool,
    ci_job_id: Uuid,
) -> Result<Vec<CiStep>, AppError> {
    let rows = sqlx::query_as::<_, CiStep>(
        "SELECT * FROM diraigent.ci_step WHERE ci_job_id = $1 ORDER BY started_at ASC NULLS LAST",
    )
    .bind(ci_job_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Create a new Forgejo integration for a project.
pub async fn create_forgejo_integration(
    pool: &PgPool,
    project_id: Uuid,
    base_url: &str,
    token: Option<&str>,
    webhook_secret: &str,
) -> Result<ForgejoIntegration, AppError> {
    let row = sqlx::query_as::<_, ForgejoIntegration>(
        "INSERT INTO diraigent.forgejo_integration (project_id, base_url, token, webhook_secret)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(project_id)
    .bind(base_url)
    .bind(token)
    .bind(webhook_secret)
    .fetch_one(pool)
    .await?;

    Ok(row)
}
