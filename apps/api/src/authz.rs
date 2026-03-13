use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::db::DiraigentDb;
use crate::models::{
    Decision, Goal, Integration, Knowledge, Observation, Plan, Report, Task, Verification, Webhook,
};

// ── ProjectScoped trait ──────────────────────────────────────────────────────

/// Trait for models that belong to a project.
///
/// Implemented for all entities that carry a `project_id`, enabling the
/// generic [`ensure_member`] and [`ensure_authority`] helpers.
pub trait ProjectScoped {
    fn project_id(&self) -> Uuid;
}

impl ProjectScoped for Task {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Goal {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Decision {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Knowledge {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Observation {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Verification {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Webhook {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Integration {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Report {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}
impl ProjectScoped for Plan {
    fn project_id(&self) -> Uuid {
        self.project_id
    }
}

/// Verify project membership and return the entity unchanged.
///
/// Replaces the two-step pattern:
/// ```ignore
/// let entity = db.get_x_by_id(id).await?;
/// require_membership(db, agent_id, user_id, entity.project_id).await?;
/// ```
pub async fn ensure_member<T: ProjectScoped>(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    entity: T,
) -> Result<T, crate::error::AppError> {
    require_membership(db, agent_id, user_id, entity.project_id()).await?;
    Ok(entity)
}

/// Verify a project authority and return the entity unchanged.
///
/// Replaces the two-step pattern:
/// ```ignore
/// let entity = db.get_x_by_id(id).await?;
/// require_authority(db, agent_id, user_id, entity.project_id, authority).await?;
/// ```
pub async fn ensure_authority_on<T: ProjectScoped>(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    entity: T,
    authority: &str,
) -> Result<T, crate::error::AppError> {
    require_authority(db, agent_id, user_id, entity.project_id(), authority).await?;
    Ok(entity)
}

/// Verify that the agent holds at least one of the given authorities, then return the entity.
pub async fn ensure_any_authority_on<T: ProjectScoped>(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    entity: T,
    authorities: &[&str],
) -> Result<T, crate::error::AppError> {
    require_any_authority(db, agent_id, user_id, entity.project_id(), authorities).await?;
    Ok(entity)
}

/// Extracts an optional agent ID from the `X-Agent-Id` header.
///
/// When the header is present the agent is validated: it must exist in the database
/// and its `owner_id` must match the authenticated user. Agents without an owner
/// (registered before ownership tracking was introduced) are allowed by any
/// authenticated user for backward compatibility.
pub struct OptionalAgentId(pub Option<Uuid>);

impl FromRequestParts<AppState> for OptionalAgentId {
    type Rejection = crate::error::AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get("X-Agent-Id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok());

        let agent_id = match raw {
            None => return Ok(OptionalAgentId(None)),
            Some(id) => id,
        };

        // Extract the authenticated user so we can verify ownership.
        let AuthUser(user_id) = AuthUser::from_request_parts(parts, state).await?;

        let is_valid = state
            .db
            .verify_agent_owner(agent_id, user_id)
            .await
            .map_err(|_| {
                crate::error::AppError::Unauthorized("Agent ownership check failed".into())
            })?;

        if !is_valid {
            crate::metrics::record_auth_failure("agent_id_spoofed");
            return Err(crate::error::AppError::Forbidden(
                "X-Agent-Id does not belong to the authenticated user".into(),
            ));
        }

        Ok(OptionalAgentId(Some(agent_id)))
    }
}

/// Verify that the caller has `manage` authority on at least one project in the tenant.
///
/// Human users pass through (implicit tenant authority). Agents must hold
/// `manage` on at least one project to mutate tenant-level resources like
/// roles and memberships.
pub async fn require_tenant_manage_authority(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    tenant_id: Uuid,
) -> Result<(), crate::error::AppError> {
    if agent_id.is_none() {
        // Human user — verify they belong to this tenant.
        let user_tenant = db.get_tenant_for_user(user_id).await.map_err(|_| {
            crate::error::AppError::Unauthorized("Failed to resolve user tenant".into())
        })?;
        match user_tenant {
            Some(t) if t.id == tenant_id => return Ok(()),
            _ => {
                crate::metrics::record_auth_failure("tenant_mismatch");
                return Err(crate::error::AppError::Forbidden(
                    "You are not a member of this tenant".into(),
                ));
            }
        }
    }
    let aid = agent_id.unwrap();
    let has = db.check_tenant_manage_authority(aid, tenant_id).await?;
    if !has {
        crate::metrics::record_auth_failure("insufficient_authority");
        return Err(crate::error::AppError::Unauthorized(
            "Agent lacks 'manage' authority on any project in this tenant".into(),
        ));
    }
    Ok(())
}

pub fn authorities_for_claim(step_name: &str) -> Vec<&'static str> {
    match step_name {
        "review" => vec!["execute", "review"],
        _ => vec!["execute"],
    }
}

pub async fn require_membership(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<(), crate::error::AppError> {
    if let Some(aid) = agent_id {
        let is_member = db.check_membership_for_agent(aid, project_id).await?;
        if !is_member {
            crate::metrics::record_auth_failure("not_a_member");
            return Err(crate::error::AppError::Forbidden(
                "Agent is not a member of this project".into(),
            ));
        }
    } else {
        // Human user (no agent context) — verify tenant alignment.
        verify_user_project_tenant(db, user_id, project_id).await?;
    }
    Ok(())
}

pub async fn require_authority(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    project_id: Uuid,
    authority: &str,
) -> Result<(), crate::error::AppError> {
    if let Some(aid) = agent_id {
        let has = db.check_authority(aid, project_id, authority).await?;
        if !has {
            crate::metrics::record_auth_failure("insufficient_authority");
            return Err(crate::error::AppError::Unauthorized(format!(
                "Agent lacks '{}' authority on this project",
                authority
            )));
        }
    } else {
        // Human user — verify tenant alignment. Human users have implicit
        // authority on all projects within their tenant.
        verify_user_project_tenant(db, user_id, project_id).await?;
    }
    Ok(())
}

pub async fn require_any_authority(
    db: &dyn DiraigentDb,
    agent_id: Option<Uuid>,
    user_id: Uuid,
    project_id: Uuid,
    authorities: &[&str],
) -> Result<(), crate::error::AppError> {
    if let Some(aid) = agent_id {
        for &auth in authorities {
            let has = db.check_authority(aid, project_id, auth).await?;
            if has {
                return Ok(());
            }
        }
        crate::metrics::record_auth_failure("insufficient_authority");
        return Err(crate::error::AppError::Unauthorized(format!(
            "Agent lacks any of {:?} authority on this project",
            authorities
        )));
    } else {
        // Human user — verify tenant alignment.
        verify_user_project_tenant(db, user_id, project_id).await?;
    }
    Ok(())
}

/// Verify that the authenticated user belongs to the same tenant as the project.
///
/// This is the fallback authorization check for human users (no `X-Agent-Id` header).
/// Without this check, any authenticated user could access any project by omitting
/// the agent ID header — a privilege escalation vulnerability.
async fn verify_user_project_tenant(
    db: &dyn DiraigentDb,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<(), crate::error::AppError> {
    let project = db.get_project_by_id(project_id).await?;
    let user_tenant = db.get_tenant_for_user(user_id).await.map_err(|_| {
        crate::error::AppError::Unauthorized("Failed to resolve user tenant".into())
    })?;
    match user_tenant {
        Some(tenant) if tenant.id == project.tenant_id => Ok(()),
        _ => {
            crate::metrics::record_auth_failure("tenant_mismatch");
            Err(crate::error::AppError::Forbidden(
                "You are not a member of this project's tenant".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_step_accepts_review_authority() {
        let auths = authorities_for_claim("review");
        assert!(auths.contains(&"review"));
        assert!(auths.contains(&"execute"));
    }

    #[test]
    fn implement_step_requires_execute_only() {
        let auths = authorities_for_claim("implement");
        assert!(auths.contains(&"execute"));
        assert!(!auths.contains(&"review"));
    }

    #[test]
    fn working_step_requires_execute_only() {
        let auths = authorities_for_claim("working");
        assert!(auths.contains(&"execute"));
        assert!(!auths.contains(&"review"));
    }

    #[test]
    fn merge_step_requires_execute_only() {
        let auths = authorities_for_claim("merge");
        assert!(auths.contains(&"execute"));
        assert!(!auths.contains(&"review"));
    }
}
