use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use diraigent_api::auth::{JwksCache, UserIdCache};
use diraigent_api::webhooks::WebhookDispatcher;
use diraigent_api::{AppState, routes};
use serde_json::Value;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower::util::ServiceExt;
use uuid::Uuid;

/// A fixed user ID used by all tests (via DEV_USER_ID env bypass).
static DEV_USER: std::sync::LazyLock<Uuid> = std::sync::LazyLock::new(Uuid::now_v7);

static INIT: std::sync::Once = std::sync::Once::new();

fn init_env() {
    INIT.call_once(|| {
        // SAFETY: called once before any tests run, no concurrent readers yet.
        unsafe {
            std::env::set_var("DEV_USER_ID", DEV_USER.to_string());
        }
    });
}

pub struct TestApp {
    pub pool: PgPool,
    admin_pool: PgPool,
    pub db_name: String,
}

/// Macro to skip a test when PostgreSQL is not available.
/// Usage: `let app = require_db!();`
macro_rules! require_db {
    () => {
        match crate::harness::TestApp::try_new().await {
            Some(app) => app,
            None => {
                eprintln!("SKIPPED: PostgreSQL not available on port 5433");
                return;
            }
        }
    };
}

impl TestApp {
    /// Try to create a TestApp; return None if the database is unreachable.
    pub async fn try_new() -> Option<Self> {
        init_env();

        let db_name = format!("test_diraigent_{}", Uuid::now_v7().simple());

        let admin_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://zivue:zivue@localhost:5433/diraigent".into());
        let admin_opts = admin_url
            .parse::<sqlx::postgres::PgConnectOptions>()
            .unwrap()
            .ssl_mode(sqlx::postgres::PgSslMode::Disable);
        let admin_pool = match PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(admin_opts)
            .await
        {
            Ok(pool) => pool,
            Err(e) => {
                eprintln!("Cannot connect to PostgreSQL (skipping test): {e}");
                return None;
            }
        };

        // Create ephemeral test database
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin_pool)
            .await
            .expect("Failed to create test database");

        // Connect to the test database with the same search_path as production
        let connect_opts = format!("postgres://zivue:zivue@localhost:5433/{db_name}")
            .parse::<sqlx::postgres::PgConnectOptions>()
            .unwrap()
            .ssl_mode(sqlx::postgres::PgSslMode::Disable)
            .options([("search_path", "public,diraigent")]);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(connect_opts)
            .await
            .expect("Failed to connect to test database");

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        // Seed the dev user into the default tenant so TenantContext works.
        // First, resolve/create the user (inserts into auth_user).
        let dev_user_id = *DEV_USER;
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO diraigent.auth_user (auth_user_id)
             VALUES ($1)
             ON CONFLICT (auth_user_id) DO UPDATE SET auth_user_id = EXCLUDED.auth_user_id
             RETURNING user_id",
        )
        .bind(dev_user_id.to_string())
        .fetch_one(&pool)
        .await
        .expect("Failed to seed dev user");

        // Add to default tenant
        let default_tenant_id: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
        sqlx::query(
            "INSERT INTO diraigent.tenant_member (tenant_id, user_id, role)
             VALUES ($1, $2, 'owner')
             ON CONFLICT (tenant_id, user_id) DO NOTHING",
        )
        .bind(default_tenant_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("Failed to add dev user to default tenant");

        Some(TestApp {
            pool,
            admin_pool,
            db_name,
        })
    }

    /// Build a fresh Router backed by this test database.
    fn router(&self) -> Router {
        let dek_cache = diraigent_api::crypto::DekCache::new();
        let raw_db: Arc<dyn diraigent_api::db::DiraigentDb> =
            Arc::new(diraigent_api::db::PostgresDb(self.pool.clone()));
        let db: Arc<dyn diraigent_api::db::DiraigentDb> =
            Arc::new(diraigent_api::db::CryptoDb::new(raw_db, dek_cache.clone()));
        let (review_tx, _) = tokio::sync::broadcast::channel(16);
        let (agent_tx, _) = tokio::sync::broadcast::channel::<diraigent_api::AgentSseEvent>(16);
        let state = AppState {
            pkg_cache: diraigent_api::package_cache::PackageCache::new(db.clone()),
            db: db.clone(),
            pool: self.pool.clone(),
            jwks: Arc::new(RwLock::new(JwksCache::default())),
            user_cache: UserIdCache::default(),
            webhooks: WebhookDispatcher::new(db),
            repo_root: None,
            is_production: false,
            projects_path: None,
            loki_url: None,
            dek_cache,
            embedder: diraigent_api::services::embeddings::create_embedder_from_env(),
            review_tx,
            agent_tx,
            sse_tickets: diraigent_api::SseTicketStore::default(),
            ws_registry: Arc::new(diraigent_api::ws_registry::WsRegistry::new()),
        };
        Router::new()
            .nest("/v1", routes::router())
            .with_state(state)
    }

    /// Send a request through the full router stack.
    pub async fn send(&self, req: Request<Body>) -> TestResponse {
        let resp = self.router().oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
        TestResponse { status, json }
    }

    /// Drop the ephemeral database.
    pub async fn cleanup(self) {
        // Close connections to the test DB before dropping it
        self.pool.close().await;

        // Terminate any remaining connections
        let _ = sqlx::query(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'",
            self.db_name,
        ))
        .execute(&self.admin_pool)
        .await;

        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\"", self.db_name))
            .execute(&self.admin_pool)
            .await;
    }

    // ── Convenience: seed data ──

    /// Create a project and return its ID.
    pub async fn create_project(&self, name: &str) -> Uuid {
        let resp = self
            .send(post_json("/v1", serde_json::json!({ "name": name })))
            .await;
        assert_eq!(resp.status, StatusCode::OK, "create project: {}", resp.json);
        resp.id()
    }

    /// Register an agent and return its ID.
    pub async fn create_agent(&self, name: &str) -> Uuid {
        let resp = self
            .send(post_json("/v1/agents", serde_json::json!({ "name": name })))
            .await;
        assert_eq!(resp.status, StatusCode::OK, "create agent: {}", resp.json);
        resp.id()
    }

    /// Create a role with given authorities and return its ID.
    pub async fn create_role(&self, project_id: Uuid, name: &str, authorities: &[&str]) -> Uuid {
        let resp = self
            .send(post_json(
                &format!("/v1/{project_id}/roles"),
                serde_json::json!({
                    "name": name,
                    "authorities": authorities,
                }),
            ))
            .await;
        assert_eq!(resp.status, StatusCode::OK, "create role: {}", resp.json);
        resp.id()
    }

    /// Add agent as member of project with given role.
    pub async fn add_member(&self, project_id: Uuid, agent_id: Uuid, role_id: Uuid) -> Uuid {
        let resp = self
            .send(post_json(
                &format!("/v1/{project_id}/members"),
                serde_json::json!({
                    "agent_id": agent_id,
                    "role_id": role_id,
                }),
            ))
            .await;
        assert_eq!(resp.status, StatusCode::OK, "add member: {}", resp.json);
        resp.id()
    }

    /// Create a task in the project.
    pub async fn create_task(&self, project_id: Uuid, title: &str) -> Value {
        let resp = self
            .send(post_json(
                &format!("/v1/{project_id}/tasks"),
                serde_json::json!({ "title": title }),
            ))
            .await;
        assert_eq!(resp.status, StatusCode::OK, "create task: {}", resp.json);
        resp.json
    }

    /// Return a synthetic playbook name for tests.
    ///
    /// The runtime now resolves playbooks from repo YAML by name; task creation no
    /// longer requires a DB playbook row to exist.
    pub async fn create_playbook(&self, step_names: &[&str]) -> String {
        let base = step_names.join("-");
        if base.is_empty() {
            "test-playbook".to_string()
        } else {
            base
        }
    }

    /// Create a task with specific fields.
    pub async fn create_task_with(&self, project_id: Uuid, body: Value) -> Value {
        let resp = self
            .send(post_json(&format!("/v1/{project_id}/tasks"), body))
            .await;
        assert_eq!(resp.status, StatusCode::OK, "create task: {}", resp.json);
        resp.json
    }
}

// ── Response helper ──

pub struct TestResponse {
    pub status: StatusCode,
    pub json: Value,
}

impl TestResponse {
    pub fn id(&self) -> Uuid {
        Uuid::parse_str(self.json["id"].as_str().unwrap()).unwrap()
    }
}

// ── Request builders ──

pub fn get(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::GET)
        .body(Body::empty())
        .unwrap()
}

pub fn post_json(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::POST)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

pub fn put_json(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::PUT)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

pub fn delete(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .method(Method::DELETE)
        .body(Body::empty())
        .unwrap()
}

/// Add X-Agent-Id header to a request.
pub fn with_agent(mut req: Request<Body>, agent_id: Uuid) -> Request<Body> {
    req.headers_mut()
        .insert("X-Agent-Id", agent_id.to_string().parse().unwrap());
    req
}
