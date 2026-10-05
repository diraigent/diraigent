//! Spectator tests use production auth and the full v1 router, not dev impersonation.
#[allow(dead_code, unused_macros)]
#[path = "integration/harness.rs"]
mod harness;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use diraigent_api::{
    AppState,
    auth::{JwksCache, UserIdCache},
    db::{CryptoDb, DiraigentDb, PostgresDb},
    routes,
};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

fn router(pool: PgPool) -> Router {
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
    Router::new()
        .nest("/v1", routes::router())
        .with_state(state)
}

async fn send(app: &Router, method: Method, url: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    if url.starts_with("/v1/spectator/") {
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn spectator_methods_and_unknown_actions_never_reach_database() {
    // No server is listening here: any accidental handler/DB execution fails this test.
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost:1/unused")
        .unwrap();
    let app = router(pool);
    let id = Uuid::nil();
    for suffix in [
        "",
        "/tasks",
        "/tasks/00000000-0000-0000-0000-000000000000",
        "/work",
        "/knowledge",
        "/decisions",
        "/tasks/claim",
        "/chat",
        "/source",
        "/logs",
        "/stream",
    ] {
        let url = format!("/v1/spectator/projects/{id}{suffix}");
        for method in [
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
        ] {
            assert_eq!(
                send(&app, method, &url).await.0,
                StatusCode::METHOD_NOT_ALLOWED,
                "{url}"
            );
        }
    }
    for url in [
        "/v1/spectator/projects",
        "/v1/spectator/agents",
        "/v1/spectator/projects/00000000-0000-0000-0000-000000000000/chat",
        "/v1/spectator/projects/00000000-0000-0000-0000-000000000000/tasks/00000000-0000-0000-0000-000000000000/claim",
    ] {
        assert_eq!(send(&app, Method::GET, url).await.0, StatusCode::NOT_FOUND);
    }
    for resource in [
        "integrations",
        "providers",
        "logs",
        "source",
        "changed-files",
        "audit",
        "webhooks",
        "streams",
        "chat",
        "agents",
        "tenants",
        "events",
        "observations",
        "reports",
        "task-logs",
    ] {
        let url = format!("/v1/spectator/projects/{id}/{resource}");
        assert_eq!(send(&app, Method::GET, &url).await.0, StatusCode::NOT_FOUND);
    }
    for suffix in [
        "tasks?limit=no",
        "knowledge?offset=-x",
        "decisions?offset=9223372036854775808",
    ] {
        assert_eq!(
            send(
                &app,
                Method::GET,
                &format!("/v1/spectator/projects/{id}/{suffix}")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    for url in [
        "/v1",
        "/v1/agents",
        "/v1/00000000-0000-0000-0000-000000000000/tasks",
    ] {
        assert_eq!(
            send(&app, Method::GET, url).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn spectator_publication_scoping_redaction_and_revocation() {
    let Some(db) = harness::TestApp::try_new().await else {
        eprintln!("SKIPPED spectator DB regression: PostgreSQL unavailable");
        return;
    };
    let public = db.create_project("Published").await;
    let private = db.create_project("Private").await;
    sqlx::query("UPDATE diraigent.project SET metadata = $2 WHERE id = $1")
        .bind(public)
        .bind(json!({"spectator_enabled": true, "secret": "hidden"}))
        .execute(&db.pool)
        .await
        .unwrap();
    let mut children = Vec::new();
    for project in [public, private] {
        let task = db.send(harness::post_json(&format!("/v1/{project}/tasks"), json!({
            "title": "Task", "context": {"spec": "Published spec", "notes": "hidden", "credentials": "hidden", "acceptance_criteria": ["Criterion", {"private": "hidden"}]}
        }))).await;
        assert_eq!(task.status, StatusCode::OK, "{}", task.json);
        children.push((project, "tasks", task.id()));
        for (collection, body) in [
            (
                "work",
                json!({"title": "Work", "description": "Published work", "metadata": {"secret": "hidden"}}),
            ),
            (
                "knowledge",
                json!({"title": "Knowledge", "content": "Published knowledge", "metadata": {"secret": "hidden"}}),
            ),
            (
                "decisions",
                json!({"title": "Decision", "context": "Published context", "consequences": "hidden"}),
            ),
        ] {
            let row = db
                .send(harness::post_json(
                    &format!("/v1/{project}/{collection}"),
                    body,
                ))
                .await;
            assert_eq!(row.status, StatusCode::OK, "{}", row.json);
            children.push((project, collection, row.id()));
        }
    }
    let app = router(db.pool.clone());
    let prefix = format!("/v1/spectator/projects/{public}");
    let (status, project) = send(&app, Method::GET, &prefix).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        project,
        json!({"id": public, "name": "Published", "description": null, "spectator": true})
    );
    let missing = send(
        &app,
        Method::GET,
        &format!("/v1/spectator/projects/{}", Uuid::now_v7()),
    )
    .await;
    assert_eq!(missing.0, StatusCode::NOT_FOUND);
    assert_eq!(
        send(
            &app,
            Method::GET,
            &format!("/v1/spectator/projects/{private}")
        )
        .await,
        missing
    );
    for (owner, collection, id) in &children {
        let result = send(&app, Method::GET, &format!("{prefix}/{collection}/{id}")).await;
        if *owner == private {
            assert_eq!(result, missing);
            continue;
        }
        assert_eq!(result.0, StatusCode::OK);
        assert!(!result.1.to_string().contains("hidden"));
        for key in [
            "project_id",
            "tenant_id",
            "metadata",
            "created_by",
            "context",
            "cost_usd",
        ] {
            if key == "context" && *collection == "decisions" {
                continue;
            }
            assert!(result.1.get(key).is_none(), "{collection}: {key}");
        }
        assert_eq!(
            send(
                &app,
                Method::GET,
                &format!("{prefix}/{collection}/{}", Uuid::now_v7())
            )
            .await,
            missing
        );
        let (status, list) = send(
            &app,
            Method::GET,
            &format!("{prefix}/{collection}?project_id={private}&limit=999&offset=0"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["limit"], 100);
        assert_eq!(list["total"], 1);
        assert_eq!(list["data"][0]["id"], json!(id));
        assert!(!list.to_string().contains("hidden"));
        let (_, list) = send(
            &app,
            Method::GET,
            &format!("{prefix}/{collection}?limit=0&offset=1"),
        )
        .await;
        assert_eq!(list["limit"], 1);
        assert_eq!(list["data"], json!([]));
        assert_eq!(
            send(
                &app,
                Method::GET,
                &format!("{prefix}/{collection}?offset=-1")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    // Real GETs must not create any users, tickets, memberships or events.
    let before: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM diraigent.auth_user), (SELECT count(*) FROM diraigent.tenant_member), (SELECT count(*) FROM diraigent.event)")
        .fetch_one(&db.pool).await.unwrap();
    send(&app, Method::GET, &format!("{prefix}/tasks")).await;
    let after: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM diraigent.auth_user), (SELECT count(*) FROM diraigent.tenant_member), (SELECT count(*) FROM diraigent.event)")
        .fetch_one(&db.pool).await.unwrap();
    assert_eq!(before, after);
    // A foreign encrypted entity must remain a 404, not attempt CryptoDb/DEK access.
    let encrypted_tenant: Uuid = sqlx::query_scalar("INSERT INTO diraigent.tenant (name, slug, encryption_mode) VALUES ('Private encrypted tenant', 'private-encrypted', 'passphrase') RETURNING id")
        .fetch_one(&db.pool).await.unwrap();
    sqlx::query("UPDATE diraigent.project SET tenant_id = $2, metadata = '{\"spectator_enabled\":true}' WHERE id = $1")
        .bind(private).bind(encrypted_tenant).execute(&db.pool).await.unwrap();
    for (_, collection, id) in children.iter().filter(|(owner, _, _)| *owner == private) {
        assert_eq!(
            send(&app, Method::GET, &format!("{prefix}/{collection}/{id}")).await,
            missing
        );
    }
    for flag in [json!(false), json!("true"), json!(null)] {
        sqlx::query("UPDATE diraigent.project SET metadata = $2 WHERE id = $1")
            .bind(public)
            .bind(json!({"spectator_enabled": flag}))
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(send(&app, Method::GET, &prefix).await, missing);
        assert_eq!(
            send(&app, Method::GET, &format!("{prefix}/tasks")).await,
            missing
        );
        for (_, collection, id) in children.iter().filter(|(owner, _, _)| *owner == public) {
            assert_eq!(
                send(&app, Method::GET, &format!("{prefix}/{collection}/{id}")).await,
                missing
            );
        }
    }
    sqlx::query(
        "UPDATE diraigent.project SET metadata = '{\"spectator_enabled\":true}' WHERE id = $1",
    )
    .bind(public)
    .execute(&db.pool)
    .await
    .unwrap();
    for mode in ["login_derived", "passphrase"] {
        sqlx::query("UPDATE diraigent.tenant SET encryption_mode = $2 WHERE id = (SELECT tenant_id FROM diraigent.project WHERE id = $1)")
            .bind(public).bind(mode).execute(&db.pool).await.unwrap();
        assert_eq!(send(&app, Method::GET, &prefix).await, missing);
        assert_eq!(
            send(&app, Method::GET, &format!("{prefix}/knowledge")).await,
            missing
        );
    }
    db.cleanup().await;
}

#[test]
fn spectator_openapi_is_public_and_allowlisted() {
    use utoipa::OpenApi;
    let doc = serde_json::to_value(diraigent_api::openapi::ApiDoc::openapi()).unwrap();
    let paths = doc["paths"].as_object().unwrap();
    let public: Vec<_> = paths
        .iter()
        .filter(|(url, _)| url.starts_with("/v1/spectator/"))
        .collect();
    assert_eq!(public.len(), 9);
    for (_, item) in public {
        assert_eq!(item["get"]["security"], json!([]));
        assert!(
            item["get"]["responses"]["200"]["content"]["application/json"]["schema"].is_object()
        );
        for method in ["post", "put", "delete", "patch"] {
            assert!(item.get(method).is_none());
        }
    }
    assert_eq!(
        doc["paths"]["/v1/spectator/projects/{project_id}/tasks/{id}"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["$ref"],
        "#/components/schemas/PublicTaskDetail"
    );
    assert_eq!(
        doc["paths"]["/v1/spectator/projects/{project_id}/tasks"]["get"]["responses"]["200"]["content"]
            ["application/json"]["schema"]["properties"]["data"]["items"]["$ref"],
        "#/components/schemas/PublicTask"
    );
}
