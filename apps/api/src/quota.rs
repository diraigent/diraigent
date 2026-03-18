//! Per-tenant resource quotas.
//!
//! Provides cached quota checks for resource creation (tasks, projects, agents).
//! Quotas are configured per-tenant via the `tenant` table columns.

use dashmap::DashMap;
use sqlx::PgPool;
use std::sync::LazyLock;
use std::time::Instant;
use uuid::Uuid;

use crate::error::AppError;

/// Cached resource count with a short TTL.
struct CachedCount {
    count: i64,
    fetched_at: Instant,
}

/// TTL for cached quota counts (seconds).
const CACHE_TTL_SECS: u64 = 60;

/// Per-tenant, per-resource count cache.
static QUOTA_CACHE: LazyLock<DashMap<(Uuid, &'static str), CachedCount>> =
    LazyLock::new(DashMap::new);

/// Resource types that can be quota-limited.
#[derive(Debug, Clone, Copy)]
pub enum Resource {
    Tasks,
    Projects,
    Agents,
}

impl Resource {
    fn cache_key(&self) -> &'static str {
        match self {
            Resource::Tasks => "tasks",
            Resource::Projects => "projects",
            Resource::Agents => "agents",
        }
    }
}

/// Check whether creating one more `resource` would exceed the tenant's quota.
///
/// Returns `Ok(())` if within limits, or `AppError::Forbidden` if the quota
/// is exceeded. Uses a short-lived cache (60s) to avoid per-request COUNT queries.
pub async fn check_quota(
    pool: &PgPool,
    tenant_id: Uuid,
    resource: Resource,
) -> Result<(), AppError> {
    let key = (tenant_id, resource.cache_key());

    // Check cache first
    let now = Instant::now();
    if let Some(entry) = QUOTA_CACHE.get(&key)
        && now.duration_since(entry.fetched_at).as_secs() < CACHE_TTL_SECS
    {
        let limit = get_limit(pool, tenant_id, &resource).await?;
        if limit > 0 && entry.count >= limit as i64 {
            return Err(quota_exceeded(&resource, entry.count, limit));
        }
        return Ok(());
    }

    // Cache miss or stale — fetch from DB
    let count = count_resource(pool, tenant_id, &resource).await?;
    let limit = get_limit(pool, tenant_id, &resource).await?;

    QUOTA_CACHE.insert(
        key,
        CachedCount {
            count,
            fetched_at: now,
        },
    );

    if limit > 0 && count >= limit as i64 {
        return Err(quota_exceeded(&resource, count, limit));
    }

    Ok(())
}

/// Invalidate the cached count for a resource after creation/deletion.
pub fn invalidate(tenant_id: Uuid, resource: Resource) {
    QUOTA_CACHE.remove(&(tenant_id, resource.cache_key()));
}

fn quota_exceeded(resource: &Resource, count: i64, limit: i32) -> AppError {
    AppError::Forbidden(format!(
        "tenant quota exceeded: {} {resource:?} (limit: {limit}, current: {count})",
        resource.cache_key(),
    ))
}

async fn count_resource(
    pool: &PgPool,
    tenant_id: Uuid,
    resource: &Resource,
) -> Result<i64, AppError> {
    let row: (i64,) = match resource {
        Resource::Tasks => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM diraigent.task t
                 JOIN diraigent.project p ON t.project_id = p.id
                 WHERE p.tenant_id = $1 AND t.state NOT IN ('done', 'cancelled')",
            )
            .bind(tenant_id)
            .fetch_one(pool)
            .await?
        }
        Resource::Projects => {
            sqlx::query_as("SELECT COUNT(*) FROM diraigent.project WHERE tenant_id = $1")
                .bind(tenant_id)
                .fetch_one(pool)
                .await?
        }
        Resource::Agents => {
            sqlx::query_as(
                "SELECT COUNT(*) FROM diraigent.agent a
                 JOIN diraigent.auth_user au ON a.owner_id = au.id
                 JOIN diraigent.tenant_member tm ON au.id = tm.user_id
                 WHERE tm.tenant_id = $1 AND a.status != 'revoked'",
            )
            .bind(tenant_id)
            .fetch_one(pool)
            .await?
        }
    };
    Ok(row.0)
}

/// Convenience: resolve tenant from project_id, then check quota.
pub async fn check_quota_for_project(
    pool: &PgPool,
    project_id: Uuid,
    resource: Resource,
) -> Result<(), AppError> {
    let tenant_id: Option<(Uuid,)> =
        sqlx::query_as("SELECT tenant_id FROM diraigent.project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;

    if let Some((tid,)) = tenant_id {
        check_quota(pool, tid, resource).await
    } else {
        Ok(()) // project not found — let the handler return 404
    }
}

async fn get_limit(pool: &PgPool, tenant_id: Uuid, resource: &Resource) -> Result<i32, AppError> {
    // We could cache this too, but tenant limits rarely change and
    // we're already hitting the DB for the count on cache miss.
    let col = match resource {
        Resource::Tasks => "max_tasks",
        Resource::Projects => "max_projects",
        Resource::Agents => "max_agents",
    };
    let query = format!("SELECT {col} FROM diraigent.tenant WHERE id = $1");
    let row: (i32,) = sqlx::query_as(&query)
        .bind(tenant_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}
