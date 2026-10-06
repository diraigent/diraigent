use crate::{
    AppState,
    auth::AuthUser,
    authz::{OptionalAgentId, require_authority, require_membership},
    error::AppError,
    models::*,
    tenant::TenantContext,
};
use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, State},
    http::{StatusCode, request::Parts},
    routing::{get, post},
};
use uuid::Uuid;

/// Resolve the credential identity, not just the optional agent header. A dak key
/// without X-Agent-Id must never acquire the owner's implicit human authority.
struct McpCaller {
    user: Uuid,
    agent: Option<Uuid>,
}
impl FromRequestParts<AppState> for McpCaller {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        let token = parts
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "));
        if let Some(key) = token.filter(|k| k.starts_with("dak_")) {
            let (agent, owner) = state
                .db
                .authenticate_agent_key(&crate::repository::hash_api_key(key))
                .await?
                .ok_or_else(|| AppError::Unauthorized("Invalid agent credential".into()))?;
            if owner != user {
                return Err(AppError::Forbidden("Credential identity mismatch".into()));
            }
            if let Some(header) = parts.headers.get("X-Agent-Id")
                && header.to_str().ok().and_then(|s| Uuid::parse_str(s).ok()) != Some(agent)
            {
                return Err(AppError::Forbidden("Agent identity mismatch".into()));
            }
            return Ok(Self {
                user,
                agent: Some(agent),
            });
        }
        if let Some(header) = parts.headers.get("X-Agent-Id")
            && header
                .to_str()
                .ok()
                .and_then(|s| Uuid::parse_str(s).ok())
                .is_none()
        {
            return Err(AppError::Validation("Invalid agent identity".into()));
        }
        let OptionalAgentId(agent) = OptionalAgentId::from_request_parts(parts, state).await?;
        Ok(Self { user, agent })
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/mcp-servers", get(list).post(create))
        .route("/{project_id}/mcp-servers/resolve", get(resolve))
        .route(
            "/{project_id}/mcp-servers/{id}/credentials/resolve",
            post(resolve_credentials),
        )
        .route(
            "/{project_id}/mcp-servers/{id}",
            get(get_one).put(update).delete(remove),
        )
        .route(
            "/{project_id}/mcp-servers/{id}/credentials",
            axum::routing::put(credentials),
        )
        .route("/{project_id}/mcp-servers/{id}/approve", post(approve))
        .route("/{project_id}/mcp-servers/{id}/disable", post(disable))
}

pub(crate) fn validate_configuration(config: &McpConfiguration) -> Result<(), AppError> {
    let invalid = || {
        AppError::Validation(
            "Invalid MCP configuration; use explicit transport and exact tool/credential names"
                .into(),
        )
    };
    if config.name.trim().is_empty()
        || config.name.len() > 128
        || config.tools.len() > 256
        || config.credential_bindings.len() > 128
    {
        return Err(invalid());
    }
    match &config.transport {
        McpTransport::Stdio {
            executable,
            arguments,
        } => {
            if !std::path::Path::new(executable).is_absolute()
                || executable.contains('\0')
                || arguments.len() > 128
                || arguments.iter().any(|a| a.len() > 4096 || a.contains('\0'))
            {
                return Err(invalid());
            }
        }
        McpTransport::StreamableHttp { endpoint } => {
            let url = reqwest::Url::parse(endpoint).map_err(|_| invalid())?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(invalid());
            }
        }
    }
    let mut names = std::collections::HashSet::new();
    for tool in &config.tools {
        if !exact_name(&tool.name) || !names.insert(&tool.name) {
            return Err(invalid());
        }
    }
    for (binding, key) in &config.credential_bindings {
        if !exact_name(binding) || !exact_name(key) {
            return Err(invalid());
        }
    }
    Ok(())
}
fn exact_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}

async fn authorize(
    state: &AppState,
    caller: &McpCaller,
    tenant: &TenantContext,
    project: Uuid,
    manage: bool,
) -> Result<(), AppError> {
    if state.db.get_project_by_id(project).await?.tenant_id != tenant.tenant_id {
        return Err(AppError::Forbidden(
            "Project belongs to another tenant".into(),
        ));
    }
    if manage {
        require_authority(
            state.db.as_ref(),
            caller.agent,
            caller.user,
            project,
            "manage",
        )
        .await
    } else {
        require_membership(state.db.as_ref(), caller.agent, caller.user, project).await
    }
}
fn audit(state: &AppState, caller: &McpCaller, server: &McpServer, action: &str) {
    // Never snapshot user-provided configuration or credential values.
    state.fire_event(server.project_id, action, "mcp_server", server.id, caller.agent, Some(caller.user), serde_json::json!({"id":server.id,"revision":server.revision,"enabled":server.enabled,"approved_revision":server.approved_revision}));
}
async fn find(state: &AppState, project: Uuid, id: Uuid) -> Result<McpServer, AppError> {
    state.db.get_mcp_server(project, id).await
}
async fn list(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path(project): Path<Uuid>,
) -> Result<Json<Vec<McpServer>>, AppError> {
    authorize(&state, &caller, &tenant, project, false).await?;
    Ok(Json(state.db.list_mcp_servers(project).await?))
}
async fn get_one(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<McpServer>, AppError> {
    authorize(&state, &caller, &tenant, project, false).await?;
    Ok(Json(find(&state, project, id).await?))
}
async fn create(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path(project): Path<Uuid>,
    Json(config): Json<McpConfiguration>,
) -> Result<(StatusCode, Json<McpServer>), AppError> {
    authorize(&state, &caller, &tenant, project, true).await?;
    validate_configuration(&config)?;
    // Creating a server cannot also grant permissions.
    if !config.tools.is_empty() {
        return Err(AppError::Validation(
            "New MCP servers must have an empty tool allowlist".into(),
        ));
    }
    let server = state.db.create_mcp_server(project, &config).await?;
    audit(&state, &caller, &server, "created");
    Ok((StatusCode::CREATED, Json(server)))
}
async fn update(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<McpServerUpdate>,
) -> Result<Json<McpServer>, AppError> {
    authorize(&state, &caller, &tenant, project, true).await?;
    validate_configuration(&req.configuration)?;
    let server = state.db.update_mcp_server(project, id, &req).await?;
    audit(&state, &caller, &server, "updated");
    Ok(Json(server))
}
async fn credentials(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<McpCredentialWrite>,
) -> Result<Json<McpServer>, AppError> {
    authorize(&state, &caller, &tenant, project, true).await?;
    if req.credentials.len() > 128
        || req
            .credentials
            .iter()
            .any(|(k, v)| !exact_name(k) || v.len() > 16384)
    {
        return Err(AppError::Validation(
            "Invalid MCP credential keys or size".into(),
        ));
    }
    let keys = req.credentials.keys().cloned().collect::<Vec<_>>();
    let secret = McpSecret(
        serde_json::to_value(&req.credentials)
            .map_err(|_| AppError::Validation("Invalid MCP credentials".into()))?,
    );
    let server = state
        .db
        .write_mcp_credentials(project, id, req.revision, &keys, &secret)
        .await?;
    audit(&state, &caller, &server, "updated");
    Ok(Json(server))
}
async fn approve(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<McpRevision>,
) -> Result<Json<McpServer>, AppError> {
    if caller.agent.is_some() {
        return Err(AppError::Forbidden(
            "Only humans may approve MCP servers".into(),
        ));
    }
    authorize(&state, &caller, &tenant, project, true).await?;
    let server = state
        .db
        .set_mcp_approval(project, id, req.revision, Some(caller.user))
        .await?;
    audit(&state, &caller, &server, "updated");
    Ok(Json(server))
}
async fn disable(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<McpRevision>,
) -> Result<Json<McpServer>, AppError> {
    authorize(&state, &caller, &tenant, project, true).await?;
    let server = state
        .db
        .set_mcp_approval(project, id, req.revision, None)
        .await?;
    audit(&state, &caller, &server, "updated");
    Ok(Json(server))
}
async fn remove(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    authorize(&state, &caller, &tenant, project, true).await?;
    let server = find(&state, project, id).await?;
    state.db.delete_mcp_server(project, id).await?;
    audit(&state, &caller, &server, "deleted");
    Ok(StatusCode::NO_CONTENT)
}
async fn resolve(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path(project): Path<Uuid>,
) -> Result<Json<McpResolution>, AppError> {
    if caller.agent.is_none() {
        return Err(AppError::Forbidden(
            "MCP resolution requires agent authentication".into(),
        ));
    }
    authorize(&state, &caller, &tenant, project, false).await?;
    require_authority(
        state.db.as_ref(),
        caller.agent,
        caller.user,
        project,
        "execute",
    )
    .await?;
    Ok(Json(McpResolution {
        contract_version: 1,
        servers: state
            .db
            .list_mcp_servers(project)
            .await?
            .into_iter()
            .filter(|s| {
                s.enabled && s.approved_revision == Some(s.revision) && s.approved_by.is_some()
            })
            .collect(),
    }))
}

async fn resolve_credentials(
    State(state): State<AppState>,
    caller: McpCaller,
    tenant: TenantContext,
    Path((project, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<McpRevision>,
) -> Result<(axum::http::HeaderMap, Json<serde_json::Value>), AppError> {
    if caller.agent.is_none() {
        return Err(AppError::Forbidden(
            "MCP credentials require agent authentication".into(),
        ));
    }
    authorize(&state, &caller, &tenant, project, false).await?;
    require_authority(
        state.db.as_ref(),
        caller.agent,
        caller.user,
        project,
        "execute",
    )
    .await?;
    let server = state.db.get_mcp_server(project, id).await?;
    if server.revision != req.revision {
        return Err(AppError::Conflict("MCP revision is stale".into()));
    }
    let mut secret = state
        .db
        .resolve_mcp_credentials(project, id, req.revision)
        .await?;
    // Supply only the keys used by this approved configuration, not unused
    // credentials retained after a configuration replacement.
    if let Some(values) = secret.0.as_object_mut() {
        values.retain(|key, _| {
            server
                .configuration
                .credential_bindings
                .values()
                .any(|binding| binding == key)
        });
    }
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok((headers, Json(secret.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_and_exact_permissions_are_validated() {
        let mut config: McpConfiguration=serde_json::from_value(serde_json::json!({"name":"test","transport":{"transport":"stdio","executable":"/usr/bin/server"}})).unwrap();
        assert!(validate_configuration(&config).is_ok());
        config.tools = vec![McpToolPermission {
            name: "*".into(),
            access: McpToolAccess::Read,
        }];
        assert!(validate_configuration(&config).is_err());
        config.tools.clear();
        for endpoint in [
            "http://example.com/mcp",
            "https://user:secret@example.com/mcp",
            "https://example.com/mcp?token=secret",
            "https://example.com/mcp#secret",
        ] {
            config.transport = McpTransport::StreamableHttp {
                endpoint: endpoint.into(),
            };
            assert!(validate_configuration(&config).is_err());
        }
        config.transport = McpTransport::StreamableHttp {
            endpoint: "https://example.com/mcp".into(),
        };
        assert!(validate_configuration(&config).is_ok());
    }
}
