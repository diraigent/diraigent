use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;

/// Resolved authenticated user. Extracted from JWT → local auth_user table.
pub struct AuthUser(pub Uuid);

/// In-memory cache for auth_user_id → user_id, avoiding per-request profile DB lookups.
const USER_CACHE_TTL_SECS: u64 = 300; // 5 minutes

#[derive(Clone, Default)]
pub struct UserIdCache {
    inner: Arc<RwLock<HashMap<String, (Uuid, Instant)>>>,
}

impl UserIdCache {
    pub async fn get(&self, auth_user_id: &str) -> Option<Uuid> {
        let cache = self.inner.read().await;
        cache.get(auth_user_id).and_then(|(id, inserted)| {
            if inserted.elapsed().as_secs() < USER_CACHE_TTL_SECS {
                Some(*id)
            } else {
                None
            }
        })
    }

    pub async fn set(&self, auth_user_id: String, user_id: Uuid) {
        let mut cache = self.inner.write().await;
        // Evict stale entries if cache is getting large
        if cache.len() > 10_000 {
            cache.retain(|_, (_, inserted)| inserted.elapsed().as_secs() < USER_CACHE_TTL_SECS);
        }
        cache.insert(auth_user_id, (user_id, Instant::now()));
    }
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: String,
}

#[derive(Debug, Clone)]
pub struct JwksKey {
    pub kid: String,
    pub n: String,
    pub e: String,
}

#[derive(Debug, Clone, Default)]
pub struct JwksCache {
    pub keys: Vec<JwksKey>,
}

pub async fn fetch_jwks(jwks_url: &str) -> anyhow::Result<JwksCache> {
    #[derive(Deserialize)]
    struct JwksResponse {
        keys: Vec<JwksKeyJson>,
    }
    #[derive(Deserialize)]
    struct JwksKeyJson {
        kid: Option<String>,
        kty: String,
        n: Option<String>,
        e: Option<String>,
    }

    let resp: JwksResponse = reqwest::get(jwks_url).await?.json().await?;
    let keys = resp
        .keys
        .into_iter()
        .filter(|k| k.kty == "RSA" && k.n.is_some() && k.e.is_some())
        .map(|k| JwksKey {
            kid: k.kid.unwrap_or_default(),
            n: k.n.unwrap(),
            e: k.e.unwrap(),
        })
        .collect();

    Ok(JwksCache { keys })
}

pub fn spawn_jwks_refresh(jwks: Arc<RwLock<JwksCache>>, jwks_url: String) {
    tokio::spawn(async move {
        loop {
            // Use a shorter interval when cache is empty (startup failure / key rotation)
            let has_keys = !jwks.read().await.keys.is_empty();
            let wait = if has_keys {
                std::time::Duration::from_secs(3600)
            } else {
                tracing::info!("JWKS cache is empty, retrying in 30s");
                std::time::Duration::from_secs(30)
            };
            tokio::time::sleep(wait).await;

            match fetch_jwks(&jwks_url).await {
                Ok(new_cache) => {
                    let count = new_cache.keys.len();
                    if count == 0 {
                        tracing::warn!("JWKS endpoint returned 0 keys");
                    } else {
                        tracing::info!(keys = count, "JWKS refreshed");
                    }
                    *jwks.write().await = new_cache;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to refresh JWKS");
                }
            }
        }
    });
}

pub fn jwks_url() -> Option<String> {
    // Treat empty string as unset (docker-compose sets `${VAR:-}` to empty string
    // when the host variable is absent).
    if let Some(url) = env::var("AUTH_JWKS_URL").ok().filter(|s| !s.is_empty()) {
        return Some(url);
    }
    let issuer = env::var("AUTH_ISSUER")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    if issuer.is_empty() {
        return None;
    }
    let base = issuer.trim_end_matches('/');
    Some(format!("{base}/jwks/"))
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Both dev bypasses are disabled when PRODUCTION=true to prevent
        // impersonation in internet-exposed deployments.
        let is_production = state.is_production;

        // Env-var bypass: DEV_USER_ID skips JWT validation (dev/local only).
        // An empty DEV_USER_ID (e.g. docker-compose default `${DEV_USER_ID:-}`)
        // is treated as unset to avoid breaking JWT auth when the variable is
        // present but has no value.
        if !is_production
            && let Ok(dev_id) = env::var("DEV_USER_ID")
            && !dev_id.is_empty()
        {
            let user_id = Uuid::parse_str(&dev_id)
                .map_err(|_| AppError::Unauthorized("Invalid DEV_USER_ID format".into()))?;
            // Ensure the auth_user row exists so tenant_member FK won't fail.
            let _ = state.db.ensure_dev_user(user_id).await;
            return Ok(AuthUser(user_id));
        }

        // Header-based dev auth: X-Dev-User-Id header bypasses JWT validation.
        // Used by desktop app and agents that don't need OAuth (dev/local only).
        if !is_production
            && let Some(dev_header) = parts
                .headers
                .get("X-Dev-User-Id")
                .and_then(|v| v.to_str().ok())
        {
            let user_id = Uuid::parse_str(dev_header)
                .map_err(|_| AppError::Unauthorized("Invalid X-Dev-User-Id format".into()))?;
            // Ensure the auth_user row exists so tenant_member FK won't fail.
            let _ = state.db.ensure_dev_user(user_id).await;
            return Ok(AuthUser(user_id));
        }

        // Extract token from Authorization header only.
        // SSE connections use a short-lived opaque ticket exchanged via
        // POST /review/stream/ticket — the full JWT must never appear in a URL.
        let token_owned: Option<String> = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer ").map(str::to_string));

        let token_str = token_owned.ok_or_else(|| {
            crate::metrics::record_auth_failure("missing_header");
            AppError::Unauthorized("Missing Authorization header".into())
        })?;
        let token = token_str.as_str();

        // Agent API key: dak_... → look up by hash, resolve to owner_id.
        if token.starts_with("dak_") {
            let key_hash = crate::repository::hash_api_key(token);
            let (_, owner_id) = state
                .db
                .authenticate_agent_key(&key_hash)
                .await
                .map_err(|_| {
                    crate::metrics::record_auth_failure("agent_key_lookup_failed");
                    AppError::Unauthorized("Agent key lookup failed".into())
                })?
                .ok_or_else(|| {
                    crate::metrics::record_auth_failure("invalid_agent_key");
                    AppError::Unauthorized("Invalid or revoked agent API key".into())
                })?;
            return Ok(AuthUser(owner_id));
        }

        let jwt_header = decode_header(token)
            .map_err(|e| AppError::Unauthorized(format!("Invalid token header: {}", e)))?;

        let kid = jwt_header.kid.unwrap_or_default();

        let cache = state.jwks.read().await;
        let key = cache
            .keys
            .iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| AppError::Unauthorized("Unknown signing key".into()))?;

        let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
            .map_err(|e| AppError::Unauthorized(format!("Invalid RSA key: {}", e)))?;

        let mut validation = Validation::new(Algorithm::RS256);
        // Only set issuer constraint when AUTH_ISSUER is non-empty.
        // Docker-compose sets `${AUTH_ISSUER:-}` to empty string when unset;
        // calling set_issuer(&[""]) would reject all real JWTs.
        if let Some(issuer) = env::var("AUTH_ISSUER").ok().filter(|s| !s.is_empty()) {
            validation.set_issuer(&[issuer]);
        }
        validation.validate_aud = false;
        validation.leeway = 3;

        let token_data = decode::<JwtClaims>(token, &decoding_key, &validation).map_err(|e| {
            crate::metrics::record_auth_failure("invalid_token");
            AppError::Unauthorized(format!("Token verification failed: {}", e))
        })?;

        let auth_user_id = &token_data.claims.sub;

        // Check cache first
        if let Some(cached) = state.user_cache.get(auth_user_id).await {
            return Ok(AuthUser(cached));
        }

        let user_id = state
            .db
            .resolve_or_create_user(auth_user_id)
            .await
            .map_err(|_| {
                crate::metrics::record_auth_failure("user_lookup_failed");
                AppError::Unauthorized("Failed to resolve user".into())
            })?;

        state.user_cache.set(auth_user_id.clone(), user_id).await;

        Ok(AuthUser(user_id))
    }
}
