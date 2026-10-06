#[allow(dead_code, unused_macros)]
#[macro_use]
#[path = "integration/harness.rs"]
mod harness;

use axum::http::StatusCode;
use diraigent_api::{
    db::{CryptoDb, DiraigentDb, PostgresDb},
    models::*,
    repository,
};
use harness::*;
use serde_json::{Value, json};
use std::sync::Arc;
use utoipa::OpenApi;

#[tokio::test]
async fn unauthenticated_registry_requests_are_rejected_before_database_access() {
    use axum::{
        Router,
        body::Body,
        http::{Method, Request},
    };
    use diraigent_api::{
        AppState,
        auth::{JwksCache, UserIdCache},
        routes,
    };
    use tokio::sync::RwLock;
    use tower::ServiceExt;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost:1/unused")
        .unwrap();
    let dek_cache = diraigent_api::crypto::DekCache::new();
    let raw: Arc<dyn DiraigentDb> = Arc::new(PostgresDb(pool.clone()));
    let db: Arc<dyn DiraigentDb> = Arc::new(CryptoDb::new(raw, dek_cache.clone()));
    let (review_tx, _) = tokio::sync::broadcast::channel(16);
    let (agent_tx, _) = tokio::sync::broadcast::channel(16);
    let state = AppState {
        pkg_cache: diraigent_api::package_cache::PackageCache::new(db.clone()),
        webhooks: diraigent_api::webhooks::WebhookDispatcher::new(db.clone()),
        db,
        pool,
        jwks: Arc::new(RwLock::new(JwksCache::default())),
        user_cache: UserIdCache::default(),
        repo_root: None,
        is_production: true,
        projects_path: None,
        loki_url: None,
        dek_cache,
        embedder: diraigent_api::services::embeddings::create_embedder_from_env(),
        review_tx,
        agent_tx,
        sse_tickets: Default::default(),
        ws_registry: Arc::new(diraigent_api::ws_registry::WsRegistry::new()),
    };
    let app = Router::new()
        .nest("/v1", routes::router())
        .with_state(state);
    for (method, suffix) in [
        (Method::GET, ""),
        (Method::POST, ""),
        (Method::GET, "/resolve"),
        (Method::PUT, "/00000000-0000-0000-0000-000000000000"),
        (
            Method::POST,
            "/00000000-0000-0000-0000-000000000000/approve",
        ),
        (
            Method::PUT,
            "/00000000-0000-0000-0000-000000000000/credentials",
        ),
        (
            Method::POST,
            "/00000000-0000-0000-0000-000000000000/credentials/resolve",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/v1/{}/mcp-servers{suffix}", Uuid::nil()))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
use uuid::Uuid;

fn config() -> Value {
    json!({"name":"test","transport":{"transport":"stdio","executable":"/usr/bin/mcp-test","arguments":[]}})
}

#[test]
fn credentials_are_write_only_and_unknown_approval_fields_rejected() {
    let req: McpCredentialWrite =
        serde_json::from_value(json!({"revision":1,"credentials":{"TOKEN":"secret-marker"}}))
            .unwrap();
    assert!(!format!("{req:?}").contains("secret-marker"));
    assert!(serde_json::from_value::<McpConfiguration>(json!({"name":"test","transport":{"transport":"stdio","executable":"/bin/server"},"enabled":true})).is_err());
    assert!(
        serde_json::from_value::<McpConfiguration>(
            json!({"name":"test","transport":{"transport":"sse","endpoint":"https://example.com"}})
        )
        .is_err()
    );
    let doc = serde_json::to_value(diraigent_api::openapi::ApiDoc::openapi()).unwrap();
    for suffix in [
        "",
        "/resolve",
        "/{id}",
        "/{id}/approve",
        "/{id}/disable",
        "/{id}/credentials",
        "/{id}/credentials/resolve",
    ] {
        assert!(doc["paths"][format!("/v1/{{project_id}}/mcp-servers{suffix}")].is_object());
    }
}

#[tokio::test]
async fn registry_approval_invalidation_isolation_and_secret_redaction() {
    let app = require_db!();
    let project = app.create_project("MCP project").await;
    let other = app.create_project("Other project").await;
    let base = format!("/v1/{project}/mcp-servers");
    let created = app.send(post_json(&base, config())).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json);
    let id = created.id();
    let item = format!("{base}/{id}");
    assert_eq!(created.json["enabled"], false);
    assert_eq!(created.json["revision"], 1);
    assert_eq!(created.json["approved_by"], Value::Null);
    assert_eq!(created.json["configuration"]["tools"], json!([]));
    assert_eq!(
        app.send(get(&format!("/v1/{other}/mcp-servers/{id}")))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.send(put_json(
            &format!("/v1/{other}/mcp-servers/{id}"),
            json!({"revision":1,"configuration":config()})
        ))
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        app.send(delete(&format!("/v1/{other}/mcp-servers/{id}")))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let registered = app
        .send(post_json("/v1/agents", json!({"name":"MCP manager"})))
        .await;
    assert_eq!(registered.status, StatusCode::OK);
    let agent = registered.id();
    let key = registered.json["api_key"].as_str().unwrap();
    let role = app
        .send(post_json(
            "/v1/roles",
            json!({"name":"MCP manager","authorities":["manage","execute"]}),
        ))
        .await
        .id();
    let member = app
        .send(post_json(
            "/v1/members",
            json!({"agent_id":agent,"role_id":role}),
        ))
        .await;
    assert_eq!(member.status, StatusCode::OK, "{}", member.json);
    let outsider = app.create_agent("MCP outsider").await;
    // Registration auto-assigns a tenant role; remove it for a genuine nonmember.
    sqlx::query("DELETE FROM diraigent.membership WHERE agent_id=$1")
        .bind(outsider)
        .execute(&app.pool)
        .await
        .unwrap();
    for req in [
        get(&base),
        post_json(&base, config()),
        get(&format!("{base}/resolve")),
        post_json(
            &format!("{item}/credentials/resolve"),
            json!({"revision":1}),
        ),
    ] {
        assert!(
            !app.send(with_agent(req, outsider))
                .await
                .status
                .is_success()
        );
    }
    let mut invalid = config();
    invalid["transport"]["executable"] = json!("npx");
    assert_eq!(
        app.send(post_json(&base, invalid)).await.status,
        StatusCode::BAD_REQUEST
    );
    let mut invalid = config();
    invalid["transport"] =
        json!({"transport":"streamable_http","endpoint":"https://user:password@example.com/mcp"});
    assert_eq!(
        app.send(post_json(&base, invalid)).await.status,
        StatusCode::BAD_REQUEST
    );
    let mut invalid = config();
    invalid["tools"] = json!([{"name":"*","access":"read"}]);
    assert_eq!(
        app.send(put_json(
            &item,
            json!({"revision":1,"configuration":invalid})
        ))
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    // Omitted X-Agent-Id is not a human identity when the bearer is an agent key.
    let mut request = post_json(&format!("{item}/approve"), json!({"revision":1}));
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {key}").parse().unwrap());
    assert_eq!(app.send(request).await.status, StatusCode::FORBIDDEN);
    assert_eq!(
        app.send(with_agent(
            post_json(&format!("{item}/approve"), json!({"revision":1})),
            agent
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );

    let mut changed = config();
    changed["tools"] =
        json!([{"name":"lookup","access":"read"},{"name":"create","access":"write"}]);
    changed["credential_bindings"] = json!({"TOKEN":"token"});
    let updated = app
        .send(put_json(
            &item,
            json!({"revision":1,"configuration":changed}),
        ))
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.json["revision"], 2);
    assert_eq!(
        app.send(post_json(&format!("{item}/approve"), json!({"revision":1})))
            .await
            .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        app.send(post_json(&format!("{item}/approve"), json!({"revision":2})))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    let wrote = app
        .send(put_json(
            &format!("{item}/credentials"),
            json!({"revision":2,"credentials":{"token":"secret-marker","unused":"unused-secret-marker"}}),
        ))
        .await;
    assert_eq!(wrote.status, StatusCode::OK);
    assert!(!wrote.json.to_string().contains("secret-marker"));
    let approved = app
        .send(post_json(&format!("{item}/approve"), json!({"revision":3})))
        .await;
    assert_eq!(approved.status, StatusCode::OK);
    assert_eq!(approved.json["enabled"], true);
    let resolved = app
        .send(with_agent(get(&format!("{base}/resolve")), agent))
        .await;
    assert_eq!(resolved.status, StatusCode::OK, "{}", resolved.json);
    assert_eq!(resolved.json["contract_version"], 1);
    assert_eq!(resolved.json["servers"].as_array().unwrap().len(), 1);
    assert!(!resolved.json.to_string().contains("secret-marker"));
    let runtime = app
        .send(with_agent(
            post_json(
                &format!("{item}/credentials/resolve"),
                json!({"revision":3}),
            ),
            agent,
        ))
        .await;
    assert_eq!(runtime.status, StatusCode::OK);
    assert_eq!(runtime.json, json!({"token":"secret-marker"}));
    assert!(
        !app.send(get(&base))
            .await
            .json
            .to_string()
            .contains("secret-marker")
    );
    let revoked = app
        .send(put_json(
            &format!("{item}/credentials"),
            json!({"revision":3,"credentials":{"token":"rotated-marker"}}),
        ))
        .await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert_eq!(revoked.json["enabled"], false);
    assert_eq!(revoked.json["approved_revision"], Value::Null);
    assert_eq!(
        app.send(with_agent(
            post_json(
                &format!("{item}/credentials/resolve"),
                json!({"revision":3})
            ),
            agent
        ))
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        app.send(with_agent(get(&format!("{base}/resolve")), agent))
            .await
            .json["servers"],
        json!([])
    );

    // Every public configuration replacement invalidates a previously approved
    // revision: arguments, executable, permissions, bindings and endpoint.
    let mut revision = 4;
    let mut replacement = changed.clone();
    for change in 0..5 {
        assert_eq!(
            app.send(post_json(
                &format!("{item}/approve"),
                json!({"revision":revision})
            ))
            .await
            .status,
            StatusCode::OK
        );
        match change {
            0 => replacement["transport"]["arguments"] = json!(["--new-mode"]),
            1 => replacement["transport"]["executable"] = json!("/opt/tools/new-mcp-server"),
            2 => replacement["tools"][0]["access"] = json!("write"),
            3 => replacement["credential_bindings"] = json!({"NEW_TOKEN":"token"}),
            _ => {
                replacement["transport"] =
                    json!({"transport":"streamable_http","endpoint":"https://example.com/mcp"})
            }
        }
        let response = app
            .send(put_json(
                &item,
                json!({"revision":revision,"configuration":replacement}),
            ))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
        revision += 1;
        assert_eq!(response.json["revision"], revision);
        assert_eq!(response.json["enabled"], false);
        assert_eq!(response.json["approved_by"], Value::Null);
        assert_eq!(response.json["approved_revision"], Value::Null);
    }

    // Change the other project's tenant: every method must reject it before lookup.
    let tenant: Uuid = sqlx::query_scalar(
        "INSERT INTO diraigent.tenant(name,slug) VALUES ('Other','mcp-other') RETURNING id",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE diraigent.project SET tenant_id=$2 WHERE id=$1")
        .bind(other)
        .bind(tenant)
        .execute(&app.pool)
        .await
        .unwrap();
    let foreign = format!("/v1/{other}/mcp-servers");
    for req in [
        get(&foreign),
        post_json(&foreign, config()),
        get(&format!("{foreign}/resolve")),
        get(&format!("{foreign}/{id}")),
        delete(&format!("{foreign}/{id}")),
        put_json(
            &format!("{foreign}/{id}"),
            json!({"revision":4,"configuration":config()}),
        ),
        post_json(&format!("{foreign}/{id}/approve"), json!({"revision":4})),
        put_json(
            &format!("{foreign}/{id}/credentials"),
            json!({"revision":4,"credentials":{}}),
        ),
    ] {
        assert_eq!(
            app.send(with_agent(req, agent)).await.status,
            StatusCode::FORBIDDEN
        );
    }
    // Mutation events deliberately contain only server id/revision/status.
    let snapshots: Vec<Value>=sqlx::query_scalar("SELECT after_state FROM diraigent.audit_log WHERE entity_type='mcp_server' AND after_state IS NOT NULL").fetch_all(&app.pool).await.unwrap();
    assert!(!snapshots.is_empty());
    for snapshot in snapshots {
        assert!(!snapshot.to_string().contains("marker"));
        assert!(snapshot.get("configuration").is_none());
    }
    assert_eq!(app.send(delete(&item)).await.status, StatusCode::NO_CONTENT);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM diraigent.mcp_server_credentials WHERE server_id=$1",
    )
    .bind(id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    app.cleanup().await;
}

#[tokio::test]
async fn configured_tenant_encryption_protects_separate_credentials() {
    let app = require_db!();
    let project = app.create_project("Encrypted MCP").await;
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM diraigent.project WHERE id=$1")
        .bind(project)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    let raw = Arc::new(PostgresDb(app.pool.clone()));
    let cache = diraigent_api::crypto::DekCache::new();
    cache
        .put(tenant, diraigent_api::crypto::Dek::generate())
        .await;
    sqlx::query(
        "UPDATE diraigent.tenant SET encryption_mode='passphrase',key_salt='test-salt' WHERE id=$1",
    )
    .bind(tenant)
    .execute(&app.pool)
    .await
    .unwrap();
    let db = CryptoDb::new(raw, cache);
    let configuration: McpConfiguration = serde_json::from_value(config()).unwrap();
    let server = db.create_mcp_server(project, &configuration).await.unwrap();
    let secret = McpSecret(json!({"token":"encrypted-marker"}));
    let server = db
        .write_mcp_credentials(project, server.id, 1, &["token".into()], &secret)
        .await
        .unwrap();
    let stored: Value = sqlx::query_scalar(
        "SELECT secret FROM diraigent.mcp_server_credentials WHERE server_id=$1",
    )
    .bind(server.id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(!stored.to_string().contains("encrypted-marker"));
    let owner: Uuid = sqlx::query_scalar("SELECT owner_id FROM diraigent.project WHERE id=$1")
        .bind(project)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    db.set_mcp_approval(project, server.id, server.revision, Some(owner))
        .await
        .unwrap();
    assert_eq!(
        db.resolve_mcp_credentials(project, server.id, server.revision)
            .await
            .unwrap()
            .0,
        secret.0
    );
    assert!(
        !serde_json::to_value(
            repository::list_mcp_servers(&app.pool, project)
                .await
                .unwrap()
        )
        .unwrap()
        .to_string()
        .contains("encrypted-marker")
    );
    let actor: Uuid=sqlx::query_scalar("INSERT INTO diraigent.auth_user(auth_user_id) VALUES ('mcp-approval-actor') RETURNING user_id").fetch_one(&app.pool).await.unwrap();
    db.set_mcp_approval(project, server.id, server.revision, Some(actor))
        .await
        .unwrap();
    sqlx::query("DELETE FROM diraigent.auth_user WHERE user_id=$1")
        .bind(actor)
        .execute(&app.pool)
        .await
        .unwrap();
    let revoked = db.list_mcp_servers(project).await.unwrap().remove(0);
    assert!(!revoked.enabled);
    assert_eq!(revoked.approved_revision, None);
    assert_eq!(revoked.approved_by, None);
    app.cleanup().await;
}
