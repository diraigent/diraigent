use crate::{error::AppError, models::*};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_mcp_server(pool: &PgPool, project: Uuid, id: Uuid) -> Result<McpServer, AppError> {
    sqlx::query_as("SELECT * FROM diraigent.mcp_server WHERE project_id=$1 AND id=$2")
        .bind(project)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("MCP server not found in project".into()))
}

pub async fn resolve_mcp_credentials(
    pool: &PgPool,
    project: Uuid,
    id: Uuid,
    revision: i64,
) -> Result<McpSecret, AppError> {
    let secret: Option<serde_json::Value> = sqlx::query_scalar("SELECT COALESCE(c.secret, '{}'::jsonb) FROM diraigent.mcp_server s LEFT JOIN diraigent.mcp_server_credentials c ON c.server_id=s.id WHERE s.project_id=$1 AND s.id=$2 AND s.revision=$3 AND s.enabled AND s.approved_revision=s.revision AND s.approved_by IS NOT NULL")
        .bind(project).bind(id).bind(revision).fetch_optional(pool).await?;
    secret.map(McpSecret).ok_or_else(stale)
}

fn stale() -> AppError {
    AppError::Conflict("MCP server missing from project or revision is stale".into())
}

pub async fn list_mcp_servers(pool: &PgPool, project: Uuid) -> Result<Vec<McpServer>, AppError> {
    Ok(
        sqlx::query_as("SELECT * FROM diraigent.mcp_server WHERE project_id=$1 ORDER BY id")
            .bind(project)
            .fetch_all(pool)
            .await?,
    )
}
pub async fn create_mcp_server(
    pool: &PgPool,
    project: Uuid,
    config: &McpConfiguration,
) -> Result<McpServer, AppError> {
    Ok(sqlx::query_as(
        "INSERT INTO diraigent.mcp_server(project_id,configuration) VALUES ($1,$2) RETURNING *",
    )
    .bind(project)
    .bind(sqlx::types::Json(config))
    .fetch_one(pool)
    .await?)
}
pub async fn update_mcp_server(
    pool: &PgPool,
    project: Uuid,
    id: Uuid,
    req: &McpServerUpdate,
) -> Result<McpServer, AppError> {
    // Every replacement advances the revision and revokes approval, even a no-op.
    sqlx::query_as("UPDATE diraigent.mcp_server SET configuration=$4,revision=revision+1,enabled=false,approved_revision=NULL,approved_by=NULL,approved_at=NULL WHERE project_id=$1 AND id=$2 AND revision=$3 RETURNING *")
        .bind(project).bind(id).bind(req.revision).bind(sqlx::types::Json(&req.configuration)).fetch_optional(pool).await?.ok_or_else(stale)
}
pub async fn write_mcp_credentials(
    pool: &PgPool,
    project: Uuid,
    id: Uuid,
    revision: i64,
    keys: &[String],
    secret: &McpSecret,
) -> Result<McpServer, AppError> {
    let mut tx = pool.begin().await?;
    let server = sqlx::query_as("UPDATE diraigent.mcp_server SET credential_keys=$4,revision=revision+1,enabled=false,approved_revision=NULL,approved_by=NULL,approved_at=NULL WHERE project_id=$1 AND id=$2 AND revision=$3 RETURNING *")
        .bind(project).bind(id).bind(revision).bind(keys).fetch_optional(&mut *tx).await?.ok_or_else(stale)?;
    sqlx::query("INSERT INTO diraigent.mcp_server_credentials(server_id,secret) VALUES ($1,$2) ON CONFLICT(server_id) DO UPDATE SET secret=EXCLUDED.secret").bind(id).bind(&secret.0).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(server)
}
pub async fn set_mcp_approval(
    pool: &PgPool,
    project: Uuid,
    id: Uuid,
    revision: i64,
    actor: Option<Uuid>,
) -> Result<McpServer, AppError> {
    let mut tx = pool.begin().await?;
    let existing: McpServer = sqlx::query_as("SELECT * FROM diraigent.mcp_server WHERE project_id=$1 AND id=$2 AND revision=$3 FOR UPDATE").bind(project).bind(id).bind(revision).fetch_optional(&mut *tx).await?.ok_or_else(stale)?;
    if actor.is_some()
        && existing
            .configuration
            .credential_bindings
            .values()
            .any(|key| !existing.credential_keys.contains(key))
    {
        return Err(AppError::Validation(
            "MCP credential binding has no stored credential".into(),
        ));
    }
    let server = sqlx::query_as("UPDATE diraigent.mcp_server SET enabled=($2::uuid IS NOT NULL),approved_revision=CASE WHEN $2::uuid IS NULL THEN NULL ELSE revision END,approved_by=$2,approved_at=CASE WHEN $2::uuid IS NULL THEN NULL ELSE now() END WHERE id=$1 RETURNING *").bind(id).bind(actor).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(server)
}
pub async fn delete_mcp_server(pool: &PgPool, project: Uuid, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM diraigent.mcp_server WHERE project_id=$1 AND id=$2")
        .bind(project)
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("MCP server not found in project".into()));
    }
    Ok(())
}
