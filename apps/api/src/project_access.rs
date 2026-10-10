//! Human project permissions; agent authorities remain a separate model.
use crate::{AppState, error::AppError};
use axum::http::{Method, request::Parts};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn role(pool: &PgPool, user: Uuid, project: Uuid) -> Result<Option<String>, AppError> {
    Ok(
        sqlx::query_scalar("SELECT diraigent.human_project_role($1,$2)")
            .bind(user)
            .bind(project)
            .fetch_one(pool)
            .await?,
    )
}

pub fn permits(role: &str, authority: &str) -> bool {
    role == "manager"
        || (role == "editor"
            && matches!(
                authority,
                "create" | "execute" | "review" | "delegate" | "decide"
            ))
}

/// Resolve entity routes without reading request bodies or trusting supplied agent headers.
/// Unscoped endpoints fail closed for ordinary workspace members.
pub async fn authorize_request(
    state: &AppState,
    parts: &Parts,
    user: Uuid,
) -> Result<(), AppError> {
    let mut segments: Vec<_> = parts
        .uri
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if segments.first() == Some(&"v1") {
        segments.remove(0);
    }
    let read = matches!(parts.method, Method::GET | Method::HEAD);
    let first = segments.first().copied().unwrap_or("");
    // Agent discovery is read access, not workspace administration. The list
    // handler validates workspace membership and filters agents by workspace/owner;
    // status events recheck visibility for every event. Keep all other agent
    // operations subject to the existing owner/administration checks below.
    if (read && segments.as_slice() == ["agents"])
        || (parts.method == Method::POST && segments.as_slice() == ["agents", "stream", "ticket"])
    {
        return Ok(());
    }
    // Non-read-only users may create their own workspace. The account guard
    // still prevents viewers from using a new workspace to escape restrictions.
    if first == "tenants" && segments.len() == 1 && parts.method == Method::POST {
        return Ok(());
    }
    // Deployment-wide logs and paths are not owned by a newly created personal
    // workspace. Keep platform administration with the seeded primary workspace.
    if matches!(first, "logs" | "settings") || (first == "packages" && !read) {
        return require_workspace_admin(state, user, Uuid::from_u128(1)).await;
    }
    if read && first == "account" && segments.contains(&"export") {
        let restricted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM diraigent.tenant_member WHERE user_id=$1 AND role NOT IN ('owner','admin'))")
            .bind(user).fetch_one(&state.pool).await?;
        if restricted {
            return Err(AppError::Forbidden(
                "Workspace-wide export requires administration of every workspace".into(),
            ));
        }
    }
    let sensitive = segments.iter().any(|s| {
        matches!(
            *s,
            "providers" | "integrations" | "webhooks" | "keys" | "dek" | "resolve"
        )
    });
    if first == "tenants"
        && sensitive
        && let Some(tenant) = segments.get(1).and_then(|s| Uuid::parse_str(s).ok())
    {
        return require_workspace_admin(state, user, tenant).await;
    }
    if first == "providers"
        && let Some(id) = segments.get(1).and_then(|s| Uuid::parse_str(s).ok())
    {
        let config: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT tenant_id,project_id FROM diraigent.provider_config WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
        if let Some((tenant, project)) = config {
            return match project {
                Some(project) => check(state, user, project, read, true).await,
                None => require_workspace_admin(state, user, tenant).await,
            };
        }
        return Err(AppError::Forbidden("Provider access required".into()));
    }
    // Safe workspace/session discovery, with project lists filtered by their handlers.
    if read
        && (segments.is_empty()
            || matches!(first, "account" | "tenants" | "dashboard" | "packages"))
    {
        return Ok(());
    }
    if read && first == "by-slug" {
        let pid: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM diraigent.project WHERE slug=$1")
                .bind(segments.get(1).copied().unwrap_or(""))
                .fetch_optional(&state.pool)
                .await?;
        if let Some(pid) = pid {
            return check(state, user, pid, true, sensitive).await;
        }
    }
    if let Ok(pid) = Uuid::parse_str(first) {
        return check(state, user, pid, read, sensitive).await;
    }
    if first == "agents"
        && segments.get(2) == Some(&"context")
        && let Some(project) = segments.get(3).and_then(|s| Uuid::parse_str(s).ok())
    {
        return check(state, user, project, read, true).await;
    }
    if let Some(id) = segments.get(1).and_then(|s| Uuid::parse_str(s).ok()) {
        if first == "agents" && state.db.verify_agent_owner(id, user).await? {
            return Ok(());
        }
        // Static table names only. Each individual entity route must resolve its project.
        let table = match first {
            "tasks" => Some("task"),
            "work" => Some("work"),
            "knowledge" => Some("knowledge"),
            "decisions" => Some("decision"),
            "observations" => Some("observation"),
            "verifications" => Some("verification"),
            "reports" => Some("report"),
            "task-logs" => Some("task_log"),
            "integrations" => Some("integration"),
            "webhooks" => Some("webhook"),
            "events" => Some("event"),
            _ => None,
        };
        if let Some(table) = table {
            let pid: Option<Uuid> = sqlx::query_scalar(&format!(
                "SELECT project_id FROM diraigent.{table} WHERE id=$1"
            ))
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
            if let Some(pid) = pid {
                return check(state, user, pid, read, sensitive).await;
            }
        }
    }
    let tenant = if let Some(id) = parts
        .headers
        .get("X-Tenant-Id")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| Uuid::parse_str(h).ok())
    {
        Some(id)
    } else {
        state.db.get_tenant_for_user(user).await?.map(|t| t.id)
    };
    if let Some(tenant) = tenant
        && state
            .db
            .get_tenant_member_for_user(tenant, user)
            .await?
            .is_some_and(|m| matches!(m.role.as_str(), "owner" | "admin"))
    {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "Workspace administration or explicit project access is required".into(),
    ))
}

async fn check(
    state: &AppState,
    user: Uuid,
    project: Uuid,
    read: bool,
    sensitive: bool,
) -> Result<(), AppError> {
    match role(&state.pool, user, project).await? {
        Some(r) if (!sensitive || r == "manager") && (read || permits(&r, "create")) => Ok(()),
        _ => Err(AppError::Forbidden("Insufficient project access".into())),
    }
}

async fn require_workspace_admin(
    state: &AppState,
    user: Uuid,
    tenant: Uuid,
) -> Result<(), AppError> {
    if state
        .db
        .get_tenant_member_for_user(tenant, user)
        .await?
        .is_some_and(|m| matches!(m.role.as_str(), "owner" | "admin"))
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Workspace administration required".into(),
        ))
    }
}
