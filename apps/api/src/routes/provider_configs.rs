use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::models::*;
use crate::tenant::TenantContext;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Project-scoped provider configs
        .route(
            "/{project_id}/providers",
            post(create_project_config).get(list_project_configs),
        )
        // Global (tenant-level) provider configs
        .route(
            "/providers",
            post(create_global_config).get(list_global_configs),
        )
        // Shared individual-item routes (work for both project-level and global)
        .route(
            "/providers/{id}",
            get(get_config).put(update_config).delete(delete_config),
        )
        // Resolution endpoint: merge project + global into a single config
        .route(
            "/{project_id}/providers/resolve/{provider}",
            get(resolve_config),
        )
}

// ── Project-scoped endpoints ──

async fn create_project_config(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateProviderConfig>,
) -> Result<(StatusCode, Json<ProviderConfig>), AppError> {
    validation::validate_create_provider_config(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    let pc = state
        .db
        .create_provider_config(tenant.tenant_id, Some(project_id), &req)
        .await?;
    Ok((StatusCode::CREATED, Json(pc)))
}

async fn list_project_configs(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<ProviderConfigFilters>,
) -> Result<Json<PaginatedResponse<ProviderConfig>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state
            .db
            .list_provider_configs(tenant.tenant_id, Some(project_id), &filters),
        state
            .db
            .count_provider_configs(tenant.tenant_id, Some(project_id), &filters),
    )
    .await
}

// ── Global (tenant-level) endpoints ──

async fn create_global_config(
    State(state): State<AppState>,
    AuthUser(_user_id): AuthUser,
    tenant: TenantContext,
    Json(req): Json<CreateProviderConfig>,
) -> Result<(StatusCode, Json<ProviderConfig>), AppError> {
    validation::validate_create_provider_config(&req)?;
    let pc = state
        .db
        .create_provider_config(tenant.tenant_id, None, &req)
        .await?;
    Ok((StatusCode::CREATED, Json(pc)))
}

async fn list_global_configs(
    State(state): State<AppState>,
    AuthUser(_user_id): AuthUser,
    tenant: TenantContext,
    Query(filters): Query<ProviderConfigFilters>,
) -> Result<Json<PaginatedResponse<ProviderConfig>>, AppError> {
    super::paginate(
        filters.limit,
        filters.offset,
        state
            .db
            .list_provider_configs(tenant.tenant_id, None, &filters),
        state
            .db
            .count_provider_configs(tenant.tenant_id, None, &filters),
    )
    .await
}

// ── Individual-item endpoints (shared for both scopes) ──

async fn get_config(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ProviderConfig>, AppError> {
    let pc = state.db.get_provider_config(id).await?;
    // Tenant isolation
    if pc.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Provider config belongs to another tenant".into(),
        ));
    }
    // If project-scoped, verify membership
    if let Some(project_id) = pc.project_id {
        require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    }
    Ok(Json(pc))
}

async fn update_config(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProviderConfig>,
) -> Result<Json<ProviderConfig>, AppError> {
    validation::validate_update_provider_config(&req)?;
    let existing = state.db.get_provider_config(id).await?;
    if existing.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Provider config belongs to another tenant".into(),
        ));
    }
    if let Some(project_id) = existing.project_id {
        require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    }
    let pc = state.db.update_provider_config(id, &req).await?;
    Ok(Json(pc))
}

async fn delete_config(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let existing = state.db.get_provider_config(id).await?;
    if existing.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Provider config belongs to another tenant".into(),
        ));
    }
    if let Some(project_id) = existing.project_id {
        require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;
    }
    state.db.delete_provider_config(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Resolution endpoint ──

async fn resolve_config(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    tenant: TenantContext,
    Path((project_id, provider)): Path<(Uuid, String)>,
) -> Result<Json<ResolvedProviderConfig>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    // Fetch project-level config (through CryptoDb for decryption)
    let project_filter = ProviderConfigFilters {
        provider: Some(provider.clone()),
        limit: Some(1),
        offset: Some(0),
    };
    let project_configs = state
        .db
        .list_provider_configs(tenant.tenant_id, Some(project_id), &project_filter)
        .await?;
    let project_config = project_configs.into_iter().next();

    // Fetch global (tenant-level) config
    let global_filter = ProviderConfigFilters {
        provider: Some(provider.clone()),
        limit: Some(1),
        offset: Some(0),
    };
    let global_configs = state
        .db
        .list_provider_configs(tenant.tenant_id, None, &global_filter)
        .await?;
    let global_config = global_configs.into_iter().next();

    let resolved = match (project_config, global_config) {
        (Some(proj), Some(global)) => {
            let (api_key, api_key_source) = if proj.api_key.is_some() {
                (proj.api_key, Some("project".to_string()))
            } else if global.api_key.is_some() {
                (global.api_key, Some("global".to_string()))
            } else {
                (None, None)
            };
            ResolvedProviderConfig {
                provider: provider.clone(),
                api_key,
                base_url: proj.base_url.or(global.base_url),
                default_model: proj.default_model.or(global.default_model),
                api_key_source,
            }
        }
        (Some(proj), None) => ResolvedProviderConfig {
            provider: provider.clone(),
            api_key_source: proj.api_key.as_ref().map(|_| "project".to_string()),
            api_key: proj.api_key,
            base_url: proj.base_url,
            default_model: proj.default_model,
        },
        (None, Some(global)) => ResolvedProviderConfig {
            provider: provider.clone(),
            api_key_source: global.api_key.as_ref().map(|_| "global".to_string()),
            api_key: global.api_key,
            base_url: global.base_url,
            default_model: global.default_model,
        },
        (None, None) => {
            return Err(AppError::NotFound(format!(
                "No provider config found for provider '{provider}'"
            )));
        }
    };
    Ok(Json(resolved))
}
