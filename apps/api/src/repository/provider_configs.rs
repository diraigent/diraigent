use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{
    CreateProviderConfig, ProviderConfig, ProviderConfigFilters, ResolvedProviderConfig,
    UpdateProviderConfig,
};

use super::{Table, delete_by_id, fetch_by_id};

pub async fn create_provider_config(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Option<Uuid>,
    req: &CreateProviderConfig,
) -> Result<ProviderConfig, AppError> {
    let row = sqlx::query_as::<_, ProviderConfig>(
        "INSERT INTO diraigent.provider_config (tenant_id, project_id, provider, api_key, base_url, default_model)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(project_id)
    .bind(&req.provider)
    .bind(&req.api_key)
    .bind(&req.base_url)
    .bind(&req.default_model)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn get_provider_config(pool: &PgPool, id: Uuid) -> Result<ProviderConfig, AppError> {
    fetch_by_id(pool, Table::ProviderConfig, id, "Provider config not found").await
}

pub async fn list_provider_configs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Option<Uuid>,
    filters: &ProviderConfigFilters,
) -> Result<Vec<ProviderConfig>, AppError> {
    let limit = filters.limit.unwrap_or(50);
    let offset = filters.offset.unwrap_or(0);

    // When project_id is Some, list project-scoped configs.
    // When project_id is None, list global (tenant-level) configs.
    let rows = if let Some(pid) = project_id {
        sqlx::query_as::<_, ProviderConfig>(
            "SELECT * FROM diraigent.provider_config
             WHERE tenant_id = $1 AND project_id = $2
               AND ($3::text IS NULL OR provider = $3)
             ORDER BY provider LIMIT $4 OFFSET $5",
        )
        .bind(tenant_id)
        .bind(pid)
        .bind(&filters.provider)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, ProviderConfig>(
            "SELECT * FROM diraigent.provider_config
             WHERE tenant_id = $1 AND project_id IS NULL
               AND ($2::text IS NULL OR provider = $2)
             ORDER BY provider LIMIT $3 OFFSET $4",
        )
        .bind(tenant_id)
        .bind(&filters.provider)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };

    Ok(rows)
}

pub async fn count_provider_configs(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Option<Uuid>,
    filters: &ProviderConfigFilters,
) -> Result<i64, AppError> {
    let (count,): (i64,) = if let Some(pid) = project_id {
        sqlx::query_as(
            "SELECT COUNT(*) FROM diraigent.provider_config
             WHERE tenant_id = $1 AND project_id = $2
               AND ($3::text IS NULL OR provider = $3)",
        )
        .bind(tenant_id)
        .bind(pid)
        .bind(&filters.provider)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT COUNT(*) FROM diraigent.provider_config
             WHERE tenant_id = $1 AND project_id IS NULL
               AND ($2::text IS NULL OR provider = $2)",
        )
        .bind(tenant_id)
        .bind(&filters.provider)
        .fetch_one(pool)
        .await?
    };
    Ok(count)
}

pub async fn update_provider_config(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateProviderConfig,
) -> Result<ProviderConfig, AppError> {
    let current = get_provider_config(pool, id).await?;

    let api_key = req.api_key.as_deref().or(current.api_key.as_deref());
    let base_url = req.base_url.as_deref().or(current.base_url.as_deref());
    let default_model = req
        .default_model
        .as_deref()
        .or(current.default_model.as_deref());

    let row = sqlx::query_as::<_, ProviderConfig>(
        "UPDATE diraigent.provider_config
         SET api_key = $1, base_url = $2, default_model = $3, updated_at = now()
         WHERE id = $4
         RETURNING *",
    )
    .bind(api_key)
    .bind(base_url)
    .bind(default_model)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn delete_provider_config(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::ProviderConfig, id, "Provider config not found").await
}

/// Resolve a provider config by merging project-level → global (tenant-level).
/// Project fields override global fields. Missing api_key in project falls back to global.
pub async fn resolve_provider_config(
    pool: &PgPool,
    tenant_id: Uuid,
    project_id: Uuid,
    provider: &str,
) -> Result<ResolvedProviderConfig, AppError> {
    // Fetch project-level config
    let project_config = sqlx::query_as::<_, ProviderConfig>(
        "SELECT * FROM diraigent.provider_config
         WHERE project_id = $1 AND provider = $2",
    )
    .bind(project_id)
    .bind(provider)
    .fetch_optional(pool)
    .await?;

    // Fetch global (tenant-level) config
    let global_config = sqlx::query_as::<_, ProviderConfig>(
        "SELECT * FROM diraigent.provider_config
         WHERE tenant_id = $1 AND project_id IS NULL AND provider = $2",
    )
    .bind(tenant_id)
    .bind(provider)
    .fetch_optional(pool)
    .await?;

    match (project_config, global_config) {
        (Some(proj), Some(global)) => {
            // Merge: project overrides, api_key falls back to global
            let (api_key, api_key_source) = if proj.api_key.is_some() {
                (proj.api_key, Some("project".to_string()))
            } else if global.api_key.is_some() {
                (global.api_key, Some("global".to_string()))
            } else {
                (None, None)
            };
            Ok(ResolvedProviderConfig {
                provider: provider.to_string(),
                api_key,
                base_url: proj.base_url.or(global.base_url),
                default_model: proj.default_model.or(global.default_model),
                api_key_source,
            })
        }
        (Some(proj), None) => Ok(ResolvedProviderConfig {
            provider: provider.to_string(),
            api_key_source: proj.api_key.as_ref().map(|_| "project".to_string()),
            api_key: proj.api_key,
            base_url: proj.base_url,
            default_model: proj.default_model,
        }),
        (None, Some(global)) => Ok(ResolvedProviderConfig {
            provider: provider.to_string(),
            api_key_source: global.api_key.as_ref().map(|_| "global".to_string()),
            api_key: global.api_key,
            base_url: global.base_url,
            default_model: global.default_model,
        }),
        (None, None) => Err(AppError::NotFound(format!(
            "No provider config found for provider '{provider}'"
        ))),
    }
}
