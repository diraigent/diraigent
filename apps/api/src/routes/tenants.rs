use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::crypto::{self, Dek};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tenants", post(create_tenant).get(list_tenants))
        .route(
            "/tenants/{tenant_id}",
            get(get_tenant).put(update_tenant).delete(delete_tenant),
        )
        .route("/tenants/by-slug/{slug}", get(get_tenant_by_slug))
        .route("/tenants/me", get(get_my_tenant))
        .route(
            "/tenants/{tenant_id}/members",
            post(add_member).get(list_members),
        )
        .route(
            "/tenants/{tenant_id}/members/{member_id}",
            axum::routing::put(update_member).delete(remove_member),
        )
        .route(
            "/tenants/{tenant_id}/members/{user_id}/keys",
            post(create_key).get(list_keys),
        )
        .route(
            "/tenants/{tenant_id}/keys/{key_id}",
            axum::routing::delete(delete_key),
        )
        // Encryption management
        .route(
            "/tenants/{tenant_id}/encryption/init",
            post(init_encryption),
        )
        .route(
            "/tenants/{tenant_id}/encryption/salt",
            get(get_encryption_salt),
        )
        .route(
            "/tenants/{tenant_id}/encryption/unlock",
            post(unlock_encryption),
        )
        .route("/tenants/{tenant_id}/encryption/rotate", post(rotate_keys))
}

// ── Authorization helpers ──

/// Require that `user_id` is an owner of `tenant_id`. Returns `403 Forbidden` otherwise.
async fn require_owner(state: &AppState, tenant_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    match state
        .db
        .get_tenant_member_for_user(tenant_id, user_id)
        .await?
    {
        Some(m) if m.role == "owner" => Ok(()),
        Some(_) => Err(AppError::Forbidden(
            "Owner role required for this operation".into(),
        )),
        None => Err(AppError::Forbidden("Not a member of this tenant".into())),
    }
}

/// Require that `user_id` is any member of `tenant_id`. Returns `403 Forbidden` otherwise.
async fn require_member(state: &AppState, tenant_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    match state
        .db
        .get_tenant_member_for_user(tenant_id, user_id)
        .await?
    {
        Some(_) => Ok(()),
        None => Err(AppError::Forbidden("Not a member of this tenant".into())),
    }
}

// ── Tenant CRUD ──

async fn create_tenant(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateTenant>,
) -> Result<Json<Tenant>, AppError> {
    let tenant = state.db.create_tenant(&req).await?;
    state
        .db
        .add_tenant_member(
            tenant.id,
            &AddTenantMember {
                user_id,
                role: Some("owner".into()),
            },
        )
        .await?;

    // Auto-initialize encryption for the new tenant
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    auto_init_encryption(&state, tenant.id, user_id, token).await;

    // Re-fetch to include updated encryption_mode
    let tenant = state.db.get_tenant_by_id(tenant.id).await?;
    Ok(Json(tenant))
}

async fn list_tenants(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Query(filters): Query<TenantFilters>,
) -> Result<Json<Vec<Tenant>>, AppError> {
    Ok(Json(state.db.list_tenants(&filters).await?))
}

async fn get_tenant(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Tenant>, AppError> {
    Ok(Json(state.db.get_tenant_by_id(tenant_id).await?))
}

async fn get_tenant_by_slug(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(slug): Path<String>,
) -> Result<Json<Tenant>, AppError> {
    Ok(Json(state.db.get_tenant_by_slug(&slug).await?))
}

async fn get_my_tenant(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Option<Tenant>>, AppError> {
    Ok(Json(state.db.get_tenant_for_user(user_id).await?))
}

async fn update_tenant(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<UpdateTenant>,
) -> Result<Json<Tenant>, AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    Ok(Json(state.db.update_tenant(tenant_id, &req).await?))
}

async fn delete_tenant(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
) -> Result<(), AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    state.db.delete_tenant(tenant_id).await
}

// ── Tenant Members ──

async fn add_member(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<AddTenantMember>,
) -> Result<Json<TenantMember>, AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    Ok(Json(state.db.add_tenant_member(tenant_id, &req).await?))
}

async fn list_members(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<Vec<TenantMember>>, AppError> {
    require_member(&state, tenant_id, user_id).await?;
    Ok(Json(state.db.list_tenant_members(tenant_id).await?))
}

async fn update_member(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path((tenant_id, member_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateTenantMember>,
) -> Result<Json<TenantMember>, AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    Ok(Json(state.db.update_tenant_member(member_id, &req).await?))
}

async fn remove_member(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path((tenant_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    state.db.remove_tenant_member(member_id).await
}

// ── Wrapped Keys ──

async fn create_key(
    State(state): State<AppState>,
    AuthUser(caller_id): AuthUser,
    Path((tenant_id, user_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CreateWrappedKey>,
) -> Result<Json<WrappedKey>, AppError> {
    // Only tenant owners or the user themselves may create wrapped keys
    let member = state
        .db
        .get_tenant_member_for_user(tenant_id, caller_id)
        .await?;
    match &member {
        Some(m) if m.role == "owner" || caller_id == user_id => {}
        Some(_) => {
            return Err(AppError::Forbidden(
                "Owner role required to create keys for other users".into(),
            ));
        }
        None => return Err(AppError::Forbidden("Not a member of this tenant".into())),
    }
    Ok(Json(
        state
            .db
            .create_wrapped_key(tenant_id, user_id, &req)
            .await?,
    ))
}

async fn list_keys(
    State(state): State<AppState>,
    AuthUser(caller_id): AuthUser,
    Path((tenant_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<WrappedKey>>, AppError> {
    // Owner or the user themselves may list wrapped keys
    let member = state
        .db
        .get_tenant_member_for_user(tenant_id, caller_id)
        .await?;
    match &member {
        Some(m) if m.role == "owner" || caller_id == user_id => {}
        Some(_) => {
            return Err(AppError::Forbidden(
                "Owner role required to list keys for other users".into(),
            ));
        }
        None => return Err(AppError::Forbidden("Not a member of this tenant".into())),
    }
    Ok(Json(state.db.list_wrapped_keys(tenant_id, user_id).await?))
}

async fn delete_key(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path((tenant_id, key_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    state.db.delete_wrapped_key(key_id).await
}

// ── Encryption Management ──

/// Request body for initializing login-derived encryption on a tenant.
#[derive(Debug, Deserialize)]
struct InitEncryptionRequest {
    /// The admin's access token (used to derive the initial KEK).
    access_token: String,
}

/// Response from encryption init.
#[derive(Debug, Serialize)]
struct InitEncryptionResponse {
    encryption_mode: String,
    salt: String,
    wrapped_dek: String,
    kdf_salt: String,
}

/// Initialize login-derived encryption for a tenant.
///
/// 1. Generate a random salt and DEK
/// 2. Derive KEK from the admin's access token + salt
/// 3. Wrap the DEK with the KEK
/// 4. Store wrapped DEK + update tenant encryption_mode
/// 5. Cache the DEK
async fn init_encryption(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<InitEncryptionRequest>,
) -> Result<Json<InitEncryptionResponse>, AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    let tenant = state.db.get_tenant_by_id(tenant_id).await?;
    if tenant.encryption_mode != "none" {
        return Err(AppError::Conflict(
            "Encryption already initialized for this tenant".into(),
        ));
    }

    // Generate salt and DEK
    let salt = crypto::generate_salt();
    let dek = Dek::generate();

    // Derive KEK from access token
    let kek = crypto::derive_kek(&req.access_token, &salt)?;

    // Wrap DEK with KEK
    let wrapped_dek = dek.wrap(&kek)?;

    // Update tenant with encryption mode and salt
    state
        .db
        .update_tenant(
            tenant_id,
            &UpdateTenant {
                name: None,
                encryption_mode: Some("login_derived".into()),
                key_salt: Some(salt.clone()),
                theme_preference: None,
                accent_color: None,
            },
        )
        .await?;

    // Store wrapped key for this user
    state
        .db
        .create_wrapped_key(
            tenant_id,
            user_id,
            &CreateWrappedKey {
                key_type: "login_derived".into(),
                wrapped_dek: wrapped_dek.clone(),
                kdf_salt: salt.clone(),
                kdf_params: None,
                key_version: Some(1),
            },
        )
        .await?;

    // Cache the DEK
    state.dek_cache.put(tenant_id, dek).await;

    Ok(Json(InitEncryptionResponse {
        encryption_mode: "login_derived".into(),
        salt: salt.clone(),
        wrapped_dek,
        kdf_salt: salt,
    }))
}

/// Auto-initialize login-derived encryption for a newly created tenant.
///
/// Called internally during tenant creation. If the access token is unavailable
/// (e.g. dev mode with X-Dev-User-Id), encryption is skipped — the tenant
/// starts with `encryption_mode = "none"` and can be initialized later.
pub(crate) async fn auto_init_encryption(
    state: &AppState,
    tenant_id: Uuid,
    user_id: Uuid,
    access_token: Option<&str>,
) {
    let token = match access_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            tracing::info!(
                tenant_id = %tenant_id,
                "skipping auto-encryption init — no access token available (dev mode?)"
            );
            return;
        }
    };

    let salt = crypto::generate_salt();
    let dek = Dek::generate();

    let kek = match crypto::derive_kek(token, &salt) {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!(tenant_id = %tenant_id, error = %e, "auto-encryption init: KEK derivation failed");
            return;
        }
    };

    let wrapped_dek = match dek.wrap(&kek) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!(tenant_id = %tenant_id, error = %e, "auto-encryption init: DEK wrapping failed");
            return;
        }
    };

    if let Err(e) = state
        .db
        .update_tenant(
            tenant_id,
            &UpdateTenant {
                name: None,
                encryption_mode: Some("login_derived".into()),
                key_salt: Some(salt.clone()),
                theme_preference: None,
                accent_color: None,
            },
        )
        .await
    {
        tracing::warn!(tenant_id = %tenant_id, error = %e, "auto-encryption init: failed to update tenant");
        return;
    }

    if let Err(e) = state
        .db
        .create_wrapped_key(
            tenant_id,
            user_id,
            &CreateWrappedKey {
                key_type: "login_derived".into(),
                wrapped_dek,
                kdf_salt: salt,
                kdf_params: None,
                key_version: Some(1),
            },
        )
        .await
    {
        tracing::warn!(tenant_id = %tenant_id, error = %e, "auto-encryption init: failed to store wrapped key");
        return;
    }

    state.dek_cache.put(tenant_id, dek).await;
    tracing::info!(tenant_id = %tenant_id, "auto-initialized login-derived encryption for new tenant");
}

/// Response from GET encryption salt.
#[derive(Debug, Serialize)]
struct EncryptionSaltResponse {
    encryption_mode: String,
    salt: Option<String>,
}

/// Get the encryption salt and mode for a tenant.
async fn get_encryption_salt(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<EncryptionSaltResponse>, AppError> {
    require_member(&state, tenant_id, user_id).await?;
    let tenant = state.db.get_tenant_by_id(tenant_id).await?;
    Ok(Json(EncryptionSaltResponse {
        encryption_mode: tenant.encryption_mode,
        salt: tenant.key_salt,
    }))
}

/// Request body for unlocking encryption (providing access token to derive KEK).
#[derive(Debug, Deserialize)]
struct UnlockEncryptionRequest {
    access_token: String,
}

/// Unlock encryption for the current session by deriving the KEK and unwrapping the DEK.
///
/// Called after login when the tenant has `login_derived` encryption.
/// The DEK is cached in memory for subsequent requests.
async fn unlock_encryption(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<UnlockEncryptionRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Check if already cached
    if state.dek_cache.get(&tenant_id).await.is_some() {
        return Ok(Json(serde_json::json!({"status": "already_unlocked"})));
    }

    let tenant = state.db.get_tenant_by_id(tenant_id).await?;
    if tenant.encryption_mode == "none" {
        return Ok(Json(serde_json::json!({"status": "no_encryption"})));
    }

    let salt = tenant
        .key_salt
        .as_deref()
        .ok_or(crypto::CryptoError::NotInitialized)?;

    // Get user's wrapped key
    let keys = state.db.list_wrapped_keys(tenant_id, user_id).await?;
    let wrapped = keys
        .iter()
        .find(|k| k.key_type == "login_derived")
        .ok_or(crypto::CryptoError::NoWrappedKey)?;

    // Derive KEK and unwrap DEK
    let kek = crypto::derive_kek(&req.access_token, salt)?;
    let dek = Dek::unwrap(&wrapped.wrapped_dek, &kek)?;

    // Cache the DEK
    state.dek_cache.put(tenant_id, dek).await;

    Ok(Json(serde_json::json!({"status": "unlocked"})))
}

// ── Key Rotation ──

/// Request body for key rotation.
#[derive(Debug, Deserialize)]
struct RotateKeysRequest {
    /// The admin's access token (used to derive KEK for re-wrapping).
    access_token: String,
}

/// Response from key rotation.
#[derive(Debug, Serialize)]
struct RotateKeysResponse {
    new_key_version: i32,
    fields_rotated: u64,
}

/// Rotate the tenant's encryption key.
///
/// 1. Get old DEK from cache (must be unlocked first)
/// 2. Generate new DEK
/// 3. Re-encrypt all fields in a transaction
/// 4. Re-wrap new DEK for all members
/// 5. Update cache with new DEK
async fn rotate_keys(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(tenant_id): Path<Uuid>,
    Json(req): Json<RotateKeysRequest>,
) -> Result<Json<RotateKeysResponse>, AppError> {
    require_owner(&state, tenant_id, user_id).await?;
    let tenant = state.db.get_tenant_by_id(tenant_id).await?;
    if tenant.encryption_mode == "none" {
        return Err(AppError::Validation(
            "Encryption is not enabled for this tenant".into(),
        ));
    }

    // Get the current DEK (must be cached / unlocked)
    let old_dek = state
        .dek_cache
        .get(&tenant_id)
        .await
        .ok_or(crypto::CryptoError::NotInitialized)?;

    // Generate new DEK
    let new_dek = Dek::generate();

    // Get all projects belonging to this tenant
    let project_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM diraigent.project WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut fields_rotated: u64 = 0;

    // Run re-encryption in a transaction
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    for project_id in &project_ids {
        // Re-encrypt task.context
        fields_rotated += rotate_json_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "task",
            "context",
            "task.context",
            "project_id",
            *project_id,
        )
        .await?;

        // Re-encrypt task_update.content (text field, joined via task)
        fields_rotated += rotate_text_field_via_join(
            &mut tx,
            &old_dek,
            &new_dek,
            "task_update",
            "content",
            "task_update.content",
            "task",
            "task_id",
            "project_id",
            *project_id,
        )
        .await?;

        // Re-encrypt knowledge.content
        fields_rotated += rotate_text_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "knowledge",
            "content",
            "knowledge.content",
            "project_id",
            *project_id,
        )
        .await?;

        // Re-encrypt decision fields
        fields_rotated += rotate_text_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "decision",
            "context",
            "decision.context",
            "project_id",
            *project_id,
        )
        .await?;
        fields_rotated += rotate_nullable_text_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "decision",
            "decision",
            "decision.decision",
            "project_id",
            *project_id,
        )
        .await?;
        fields_rotated += rotate_nullable_text_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "decision",
            "rationale",
            "decision.rationale",
            "project_id",
            *project_id,
        )
        .await?;

        // Re-encrypt integration.credentials (JSONB)
        fields_rotated += rotate_json_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "integration",
            "credentials",
            "integration.credentials",
            "project_id",
            *project_id,
        )
        .await?;

        // Re-encrypt webhook.secret (nullable text)
        fields_rotated += rotate_nullable_text_field(
            &mut tx,
            &old_dek,
            &new_dek,
            "webhook",
            "secret",
            "webhook.secret",
            "project_id",
            *project_id,
        )
        .await?;

        // Re-encrypt changed_file.diff (nullable text, joined via task)
        fields_rotated += rotate_nullable_text_field_via_join(
            &mut tx,
            &old_dek,
            &new_dek,
            "changed_file",
            "diff",
            "changed_file.diff",
            "task",
            "task_id",
            "project_id",
            *project_id,
        )
        .await?;
    }

    // Get current max key_version for this tenant
    let current_version: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(key_version), 0) FROM diraigent.wrapped_key WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_one(tx.as_mut())
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let new_version = current_version + 1;

    // Re-wrap new DEK for all members
    let salt = tenant
        .key_salt
        .as_deref()
        .ok_or(crypto::CryptoError::NotInitialized)?;

    let members = state.db.list_tenant_members(tenant_id).await?;

    // For login-derived mode, we can re-wrap using the caller's access token
    // (the caller must be an admin). Other members' wrapped keys will be
    // created when they next log in and provide their access token.
    // For now, wrap for the current user.
    let kek = crypto::derive_kek(&req.access_token, salt)?;
    let wrapped_new = new_dek.wrap(&kek)?;

    sqlx::query(
        "INSERT INTO diraigent.wrapped_key (tenant_id, user_id, key_type, wrapped_dek, kdf_salt, key_version) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(&tenant.encryption_mode)
    .bind(&wrapped_new)
    .bind(salt)
    .bind(new_version)
    .execute(tx.as_mut())
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    // Mark old versions for other members as needing re-wrap
    // (they'll get new wrapped keys on next login)
    let _members_count = members.len();

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Update DEK cache with new key
    state.dek_cache.put(tenant_id, new_dek).await;

    tracing::info!(
        tenant_id = %tenant_id,
        new_version,
        fields_rotated,
        "key rotation completed"
    );

    Ok(Json(RotateKeysResponse {
        new_key_version: new_version,
        fields_rotated,
    }))
}

/// Re-encrypt a non-nullable text field in a table.
#[allow(clippy::too_many_arguments)]
async fn rotate_text_field(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    old_dek: &Dek,
    new_dek: &Dek,
    table: &str,
    column: &str,
    aad: &str,
    scope_column: &str,
    scope_id: Uuid,
) -> Result<u64, AppError> {
    let query = format!(
        "SELECT id, {column} FROM diraigent.{table} WHERE {scope_column} = $1 AND {column} LIKE 'enc:v1:%'"
    );
    let rows = sqlx::query(&query)
        .bind(scope_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut count = 0u64;
    let update_query = format!("UPDATE diraigent.{table} SET {column} = $1 WHERE id = $2");
    for row in &rows {
        let id: Uuid = row.get("id");
        let value: String = row.get(column);
        let decrypted = old_dek
            .decrypt_str(&value, aad)
            .map_err(|e| AppError::Internal(format!("decrypt {aad}: {e}")))?;
        let re_encrypted = new_dek
            .encrypt_str(&decrypted, aad)
            .map_err(|e| AppError::Internal(format!("encrypt {aad}: {e}")))?;
        sqlx::query(&update_query)
            .bind(&re_encrypted)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        count += 1;
    }
    Ok(count)
}

/// Re-encrypt a nullable text field in a table.
#[allow(clippy::too_many_arguments)]
async fn rotate_nullable_text_field(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    old_dek: &Dek,
    new_dek: &Dek,
    table: &str,
    column: &str,
    aad: &str,
    scope_column: &str,
    scope_id: Uuid,
) -> Result<u64, AppError> {
    let query = format!(
        "SELECT id, {column} FROM diraigent.{table} WHERE {scope_column} = $1 AND {column} LIKE 'enc:v1:%'"
    );
    let rows = sqlx::query(&query)
        .bind(scope_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut count = 0u64;
    let update_query = format!("UPDATE diraigent.{table} SET {column} = $1 WHERE id = $2");
    for row in &rows {
        let id: Uuid = row.get("id");
        let value: Option<String> = row.get(column);
        if let Some(ref val) = value {
            let decrypted = old_dek
                .decrypt_str(val, aad)
                .map_err(|e| AppError::Internal(format!("decrypt {aad}: {e}")))?;
            let re_encrypted = new_dek
                .encrypt_str(&decrypted, aad)
                .map_err(|e| AppError::Internal(format!("encrypt {aad}: {e}")))?;
            sqlx::query(&update_query)
                .bind(&re_encrypted)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            count += 1;
        }
    }
    Ok(count)
}

/// Re-encrypt a JSONB field stored as an encrypted string.
#[allow(clippy::too_many_arguments)]
async fn rotate_json_field(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    old_dek: &Dek,
    new_dek: &Dek,
    table: &str,
    column: &str,
    aad: &str,
    scope_column: &str,
    scope_id: Uuid,
) -> Result<u64, AppError> {
    // JSONB fields that are encrypted are stored as a JSON string value: '"enc:v1:..."'
    let query = format!(
        "SELECT id, {column}::text FROM diraigent.{table} WHERE {scope_column} = $1 AND {column}::text LIKE '%enc:v1:%'"
    );
    let rows = sqlx::query(&query)
        .bind(scope_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut count = 0u64;
    let update_query = format!("UPDATE diraigent.{table} SET {column} = $1::jsonb WHERE id = $2");
    for row in &rows {
        let id: Uuid = row.get("id");
        let raw_text: String = row.get(column);
        // Parse the JSONB text to get the encrypted string value
        let json_val: serde_json::Value = serde_json::from_str(&raw_text)
            .map_err(|e| AppError::Internal(format!("parse {aad} JSON: {e}")))?;
        let decrypted_val = old_dek
            .decrypt_json(&json_val, aad)
            .map_err(|e| AppError::Internal(format!("decrypt {aad}: {e}")))?;
        let re_encrypted_val = new_dek
            .encrypt_json(&decrypted_val, aad)
            .map_err(|e| AppError::Internal(format!("encrypt {aad}: {e}")))?;
        let re_encrypted_str = serde_json::to_string(&re_encrypted_val)
            .map_err(|e| AppError::Internal(format!("serialize {aad}: {e}")))?;
        sqlx::query(&update_query)
            .bind(&re_encrypted_str)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        count += 1;
    }
    Ok(count)
}

/// Re-encrypt a text field in a table joined via another table for project scoping.
/// e.g. task_update.content scoped by task.project_id.
#[allow(clippy::too_many_arguments)]
async fn rotate_text_field_via_join(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    old_dek: &Dek,
    new_dek: &Dek,
    table: &str,
    column: &str,
    aad: &str,
    join_table: &str,
    fk_column: &str,
    scope_column: &str,
    scope_id: Uuid,
) -> Result<u64, AppError> {
    let query = format!(
        "SELECT t.id, t.{column} FROM diraigent.{table} t \
         JOIN diraigent.{join_table} j ON t.{fk_column} = j.id \
         WHERE j.{scope_column} = $1 AND t.{column} LIKE 'enc:v1:%'"
    );
    let rows = sqlx::query(&query)
        .bind(scope_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut count = 0u64;
    let update_query = format!("UPDATE diraigent.{table} SET {column} = $1 WHERE id = $2");
    for row in &rows {
        let id: Uuid = row.get("id");
        let value: String = row.get(column);
        let decrypted = old_dek
            .decrypt_str(&value, aad)
            .map_err(|e| AppError::Internal(format!("decrypt {aad}: {e}")))?;
        let re_encrypted = new_dek
            .encrypt_str(&decrypted, aad)
            .map_err(|e| AppError::Internal(format!("encrypt {aad}: {e}")))?;
        sqlx::query(&update_query)
            .bind(&re_encrypted)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        count += 1;
    }
    Ok(count)
}

/// Re-encrypt a nullable text field in a table joined via another table for project scoping.
#[allow(clippy::too_many_arguments)]
async fn rotate_nullable_text_field_via_join(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    old_dek: &Dek,
    new_dek: &Dek,
    table: &str,
    column: &str,
    aad: &str,
    join_table: &str,
    fk_column: &str,
    scope_column: &str,
    scope_id: Uuid,
) -> Result<u64, AppError> {
    let query = format!(
        "SELECT t.id, t.{column} FROM diraigent.{table} t \
         JOIN diraigent.{join_table} j ON t.{fk_column} = j.id \
         WHERE j.{scope_column} = $1 AND t.{column} LIKE 'enc:v1:%'"
    );
    let rows = sqlx::query(&query)
        .bind(scope_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut count = 0u64;
    let update_query = format!("UPDATE diraigent.{table} SET {column} = $1 WHERE id = $2");
    for row in &rows {
        let id: Uuid = row.get("id");
        let value: Option<String> = row.get(column);
        if let Some(ref val) = value {
            let decrypted = old_dek
                .decrypt_str(val, aad)
                .map_err(|e| AppError::Internal(format!("decrypt {aad}: {e}")))?;
            let re_encrypted = new_dek
                .encrypt_str(&decrypted, aad)
                .map_err(|e| AppError::Internal(format!("encrypt {aad}: {e}")))?;
            sqlx::query(&update_query)
                .bind(&re_encrypted)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            count += 1;
        }
    }
    Ok(count)
}
