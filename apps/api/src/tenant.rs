//! Tenant context extractor for route handlers.
//!
//! Resolves the authenticated user's tenant membership, providing the tenant ID
//! and role for tenant-scoping queries.
//!
//! Resolution order for users without an existing membership:
//! 1. Try to join the well-known default tenant (seeded by migration 029).
//! 2. If the default tenant does not exist (e.g. fresh schema), create a
//!    personal workspace for the user and make them the owner.
//!
//! This ensures every authenticated user always has a tenant, enabling
//! first-login registration without manual setup.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::models::{AddTenantMember, CreateTenant};

/// Well-known default tenant UUID (seeded by migration 029).
const DEFAULT_TENANT_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);

/// Resolved tenant context for the authenticated user.
///
/// Extracted from the request by resolving `AuthUser` → `get_tenant_for_user`.
/// If the user has no tenant membership, the extractor registers them:
/// first by trying to join the default tenant, and falling back to creating
/// a personal workspace.
pub struct TenantContext {
    pub tenant_id: Uuid,
    pub role: String,
}

impl FromRequestParts<AppState> for TenantContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user_id) = AuthUser::from_request_parts(parts, state).await?;

        // Look up the user's existing tenant membership.
        let tenant = match state.db.get_tenant_for_user(user_id).await? {
            Some(t) => t,
            None => {
                // No membership yet — register the user into a tenant.
                register_user_tenant(state, user_id).await?
            }
        };

        // Get the member's role within the resolved tenant.
        let members = state.db.list_tenant_members(tenant.id).await?;
        let role = members
            .iter()
            .find(|m| m.user_id == user_id)
            .map(|m| m.role.clone())
            .unwrap_or_else(|| "member".into());

        Ok(TenantContext {
            tenant_id: tenant.id,
            role,
        })
    }
}

/// Register a new user into a tenant on their first request.
///
/// Attempts to join the shared default tenant first.  If the default tenant
/// does not exist (or any other constraint prevents the join), a personal
/// workspace is created and the user becomes its owner.
///
/// This is the registration path for new users: every authenticated user
/// that hits a tenant-scoped endpoint will be registered exactly once.
async fn register_user_tenant(
    state: &AppState,
    user_id: Uuid,
) -> Result<crate::models::Tenant, AppError> {
    // 1. In dev mode only, try to join the shared default tenant.
    let dev_mode = std::env::var("DEV_USER_ID").is_ok_and(|s| !s.is_empty());
    if dev_mode {
        let join_default = state
            .db
            .add_tenant_member(
                DEFAULT_TENANT_ID,
                &AddTenantMember {
                    user_id,
                    role: Some("member".into()),
                },
            )
            .await;

        if let Err(e) = join_default {
            tracing::warn!(
                user_id = %user_id,
                error = %e,
                "default tenant join failed; creating personal workspace"
            );
        } else if let Some(t) = state.db.get_tenant_for_user(user_id).await? {
            tracing::info!(user_id = %user_id, "registered user into default tenant (dev mode)");
            return Ok(t);
        }
    }

    // 2. Create a personal workspace for the user.
    //    Slug: "workspace-<first 8 hex chars of user_id>" — unique per user.
    let slug = format!("workspace-{}", &user_id.simple().to_string()[..8]);
    let workspace = state
        .db
        .create_tenant(&CreateTenant {
            name: "My Workspace".into(),
            slug: Some(slug.clone()),
        })
        .await
        .map_err(|e| {
            tracing::error!(
                user_id = %user_id,
                slug = %slug,
                error = %e,
                "failed to create personal workspace for new user"
            );
            e
        })?;

    state
        .db
        .add_tenant_member(
            workspace.id,
            &AddTenantMember {
                user_id,
                role: Some("owner".into()),
            },
        )
        .await?;

    tracing::info!(
        user_id = %user_id,
        tenant_id = %workspace.id,
        slug = %slug,
        "created personal workspace for new user"
    );

    // Auto-initialize encryption for the new personal workspace
    crate::routes::tenants::auto_init_encryption(state, workspace.id, user_id).await;

    // Re-fetch so we have the full Tenant row.
    state
        .db
        .get_tenant_for_user(user_id)
        .await?
        .ok_or_else(|| AppError::Internal("Failed to resolve tenant after registration".into()))
}

/// Optional variant — succeeds even if the user has no tenant.
pub struct OptionalTenantContext(pub Option<TenantContext>);

impl FromRequestParts<AppState> for OptionalTenantContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match TenantContext::from_request_parts(parts, state).await {
            Ok(ctx) => Ok(OptionalTenantContext(Some(ctx))),
            Err(_) => Ok(OptionalTenantContext(None)),
        }
    }
}
