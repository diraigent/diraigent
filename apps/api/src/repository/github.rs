use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::GitHubIntegration;

/// Create a new GitHub integration for a project.
pub async fn create_github_integration(
    pool: &PgPool,
    project_id: Uuid,
    base_url: &str,
    token: Option<&str>,
    webhook_secret: &str,
) -> Result<GitHubIntegration, AppError> {
    let row = sqlx::query_as::<_, GitHubIntegration>(
        "INSERT INTO diraigent.github_integration (project_id, base_url, token, webhook_secret)
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

/// Look up the GitHub integration by its own ID (used for webhook routing).
pub async fn get_github_integration(
    pool: &PgPool,
    id: Uuid,
) -> Result<GitHubIntegration, AppError> {
    sqlx::query_as::<_, GitHubIntegration>(
        "SELECT * FROM diraigent.github_integration WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("GitHub integration not found".into()))
}

/// Look up the GitHub integration for a given project.
pub async fn get_github_integration_by_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<GitHubIntegration, AppError> {
    sqlx::query_as::<_, GitHubIntegration>(
        "SELECT * FROM diraigent.github_integration WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("GitHub integration not found".into()))
}
