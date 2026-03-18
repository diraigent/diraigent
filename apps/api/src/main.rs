use axum::extract::DefaultBodyLimit;
use axum::http::{Method, Request, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router, extract::State};
use diraigent_api::{
    AppState, auth, crypto, csrf, db, metrics, package_cache, rate_limit, routes, services,
    stale_detector, webhooks, ws_registry,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = state.db.health_check().await;

    let status = if db_ok { "ok" } else { "degraded" };
    let http_status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let ws_connected = state.ws_registry.has_connections();

    (
        http_status,
        Json(json!({
            "status": status,
            "checks": {
                "database": if db_ok { "connected" } else { "disconnected" },
                "ws_agents": if ws_connected { "connected" } else { "none" },
            }
        })),
    )
}

async fn add_request_id(
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> Response<axum::body::Body> {
    let request_id = uuid::Uuid::now_v7().to_string();
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("x-request-id", request_id.parse().unwrap());
    response
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    shared_utils::server::init_tracing();
    let (config, meter_provider, start_time) =
        shared_utils::server::init_config_and_observability()?;

    let db_url = env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is required"))?;
    let connect_opts = db_url
        .parse::<sqlx::postgres::PgConnectOptions>()?
        .options([("search_path", "public,diraigent")]);
    let max_connections: u32 = env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let pool = {
        const MAX_RETRIES: u32 = 5;
        let mut attempt = 0u32;
        loop {
            match PgPoolOptions::new()
                .max_connections(max_connections)
                .acquire_timeout(Duration::from_secs(30))
                .connect_with(connect_opts.clone())
                .await
            {
                Ok(p) => break p,
                Err(e) if attempt < MAX_RETRIES => {
                    attempt += 1;
                    let wait = Duration::from_secs(2u64.pow(attempt));
                    tracing::warn!(
                        attempt,
                        max_retries = MAX_RETRIES,
                        wait_secs = wait.as_secs(),
                        error = %e,
                        "DB connection failed, retrying…"
                    );
                    tokio::time::sleep(wait).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    };
    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!("PostgreSQL connected");
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("PostgreSQL migrations applied");
    let dek_cache = crypto::DekCache::new();
    let raw_db: Arc<dyn db::DiraigentDb> = Arc::new(db::PostgresDb(pool.clone()));
    let database: Arc<dyn db::DiraigentDb> = Arc::new(db::CryptoDb::new(raw_db, dek_cache.clone()));

    if env::var("DEV_USER_ID").is_ok() {
        tracing::warn!("⚠️  DEV_USER_ID is set — JWT auth is BYPASSED. Do not use in production!");
    }

    let is_production = env::var("PRODUCTION")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    if is_production {
        tracing::info!("🔒 Production mode: X-Dev-User-Id header bypass is DISABLED");
    } else {
        tracing::warn!(
            "⚠️  Dev mode: X-Dev-User-Id header bypass is ACTIVE. Set PRODUCTION=true to disable."
        );
    }

    let jwks_url = auth::jwks_url();
    let jwks_cache = match &jwks_url {
        Some(url) => match auth::fetch_jwks(url).await {
            Ok(cache) => {
                tracing::info!(keys = cache.keys.len(), "JWKS loaded");
                cache
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch JWKS on startup, will retry");
                auth::JwksCache::default()
            }
        },
        None => {
            tracing::info!("AUTH_JWKS_URL/AUTH_ISSUER not set — JWT auth disabled (dev mode)");
            auth::JwksCache::default()
        }
    };
    let jwks = Arc::new(RwLock::new(jwks_cache));
    if let Some(url) = jwks_url {
        auth::spawn_jwks_refresh(jwks.clone(), url);
    }

    let webhook_dispatcher = webhooks::WebhookDispatcher::new(database.clone());

    let repo_root = env::var("REPO_ROOT")
        .ok()
        .map(std::path::PathBuf::from)
        .filter(|p| p.join(".git").exists());
    if let Some(ref root) = repo_root {
        tracing::info!(path = %root.display(), "Git repo root configured");
    } else {
        tracing::info!("REPO_ROOT not set — git API endpoints disabled");
    }

    let projects_path = env::var("PROJECTS_PATH").ok().map(std::path::PathBuf::from);
    if let Some(ref path) = projects_path {
        tracing::info!(path = %path.display(), "Projects path configured — repo_path values are relative to this directory");
    } else {
        tracing::info!("PROJECTS_PATH not set — project repo_path values stored as-is");
    }

    let embedder = services::embeddings::create_embedder_from_env();

    // Broadcast channel for human_review SSE notifications.
    // Capacity 64: at most 64 unread events per subscriber before oldest are dropped.
    let (review_tx, _) = tokio::sync::broadcast::channel(64);
    // Broadcast channel for agent status SSE notifications.
    let (agent_tx, _) = tokio::sync::broadcast::channel::<diraigent_api::AgentSseEvent>(64);

    let is_production = env::var("PRODUCTION")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let state = AppState {
        pkg_cache: package_cache::PackageCache::new(database.clone()),
        db: database.clone(),
        pool: pool.clone(),
        jwks,
        user_cache: auth::UserIdCache::default(),
        webhooks: webhook_dispatcher.clone(),
        repo_root,
        is_production,
        projects_path,
        loki_url: env::var("LOKI_URL").ok(),
        dek_cache,
        embedder,
        review_tx,
        agent_tx,
        sse_tickets: diraigent_api::SseTicketStore::default(),
        ws_registry: Arc::new(ws_registry::WsRegistry::new()),
    };

    stale_detector::spawn_stale_detector(database.clone(), webhook_dispatcher);
    stale_detector::spawn_observation_cleaner(database.clone());

    let cors = if let Ok(origins) = env::var("CORS_ORIGINS") {
        let origins: Vec<_> = origins
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
            ])
            .allow_headers(tower_http::cors::Any)
            .allow_origin(origins)
    } else {
        tracing::warn!(
            "CORS_ORIGINS is not set — all cross-origin requests will be denied. Set CORS_ORIGINS to a comma-separated list of allowed origins."
        );
        shared_utils::server::standard_cors()
    };

    let app = Router::new()
        .route(
            "/health/live",
            get(|| async { Json(json!({"status": "ok"})) }),
        )
        .route("/health/ready", get(readiness_check))
        .route(
            "/v1/config",
            get({
                // auth_required is true only when AUTH_ISSUER is a non-empty string AND
                // DEV_USER_ID is absent or empty (docker-compose `${VAR:-}` sets to "").
                let auth_required = env::var("AUTH_ISSUER").is_ok_and(|s| !s.is_empty())
                    && !env::var("DEV_USER_ID").is_ok_and(|s| !s.is_empty());
                let chat_model = env::var("CHAT_MODEL").unwrap_or_else(|_| "sonnet".into());
                let api_version = env::var("DIRAIGENT_VERSION")
                    .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
                move || async move {
                    Json(json!({
                        "auth_required": auth_required,
                        "chat_model": chat_model,
                        "api_version": api_version,
                    }))
                }
            }),
        )
        .nest("/v1", routes::router())
        .layer(middleware::from_fn(add_request_id))
        .layer(middleware::from_fn(metrics::record_metrics))
        .layer(DefaultBodyLimit::max(
            env::var("MAX_BODY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1024 * 1024), // 1 MB default
        ))
        .layer(middleware::from_fn(csrf::csrf_check))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit,
        ))
        .layer(cors)
        .layer(shared_utils::server::standard_trace())
        .with_state(state);

    shared_utils::server::run(&config, meter_provider, app, start_time).await
}
