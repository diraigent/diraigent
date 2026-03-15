use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CiRun, ForgejoIntegration};

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
