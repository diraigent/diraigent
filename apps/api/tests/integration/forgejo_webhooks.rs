use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

/// Build a POST request with raw bytes, custom headers for Forgejo webhook.
fn webhook_request(
    integration_id: Uuid,
    event_type: &str,
    body: &[u8],
    signature: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(format!("/v1/webhooks/forgejo/{integration_id}"))
        .method(Method::POST)
        .header("content-type", "application/json")
        .header("X-Gitea-Event", event_type);

    if let Some(sig) = signature {
        builder = builder.header("X-Gitea-Signature", sig);
    }

    builder.body(Body::from(body.to_vec())).unwrap()
}

/// Compute HMAC-SHA256 of `payload` with `secret` and return hex-encoded signature.
fn compute_signature(secret: &str, payload: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    hex::encode(mac.finalize().into_bytes())
}

/// Happy path: POST a correctly signed workflow_run payload, assert 200 and ci_run created.
#[tokio::test]
async fn webhook_workflow_run_creates_ci_run() {
    let app = require_db!();
    let project_id = app.create_project("forgejo-webhook-test").await;

    let secret = "test-webhook-secret-12345";
    let integration_id = Uuid::now_v7();

    // Insert a forgejo_integration row directly via SQL
    sqlx::query(
        "INSERT INTO diraigent.forgejo_integration (id, project_id, base_url, webhook_secret, enabled)
         VALUES ($1, $2, 'https://git.example.com', $3, true)",
    )
    .bind(integration_id)
    .bind(project_id)
    .bind(secret)
    .execute(&app.pool)
    .await
    .expect("Failed to insert forgejo_integration");

    // Build a workflow_run payload
    let payload = serde_json::json!({
        "workflow_run": {
            "id": 42,
            "name": "CI Pipeline",
            "status": "completed",
            "conclusion": "success",
            "head_branch": "main",
            "head_sha": "abc123def456",
            "triggering_actor": {
                "login": "testuser"
            },
            "run_started_at": "2026-03-15T10:00:00Z",
            "updated_at": "2026-03-15T10:05:00Z"
        }
    });
    let body = serde_json::to_vec(&payload).unwrap();
    let signature = compute_signature(secret, &body);

    let resp = app
        .send(webhook_request(
            integration_id,
            "workflow_run",
            &body,
            Some(&signature),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "webhook response: {}",
        resp.json
    );
    assert_eq!(resp.json["status"], "ok");

    // Verify a ci_run record was created
    let ci_run: (
        Uuid,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT id, workflow_name, status, branch, commit_sha, triggered_by
             FROM diraigent.ci_run
             WHERE project_id = $1 AND forgejo_run_id = $2",
    )
    .bind(project_id)
    .bind(42_i64)
    .fetch_one(&app.pool)
    .await
    .expect("ci_run record should exist after webhook");

    assert_eq!(ci_run.1, "CI Pipeline");
    assert_eq!(ci_run.2, "success"); // conclusion takes precedence over status
    assert_eq!(ci_run.3.as_deref(), Some("main"));
    assert_eq!(ci_run.4.as_deref(), Some("abc123def456"));
    assert_eq!(ci_run.5.as_deref(), Some("testuser"));

    app.cleanup().await;
}

/// Missing X-Gitea-Signature header → 401.
#[tokio::test]
async fn webhook_missing_signature_returns_401() {
    let app = require_db!();
    let project_id = app.create_project("forgejo-no-sig").await;

    let integration_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO diraigent.forgejo_integration (id, project_id, base_url, webhook_secret, enabled)
         VALUES ($1, $2, 'https://git.example.com', 'secret', true)",
    )
    .bind(integration_id)
    .bind(project_id)
    .execute(&app.pool)
    .await
    .unwrap();

    let body = b"{}";
    let resp = app
        .send(webhook_request(integration_id, "workflow_run", body, None))
        .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

/// Invalid HMAC signature → 401.
#[tokio::test]
async fn webhook_invalid_signature_returns_401() {
    let app = require_db!();
    let project_id = app.create_project("forgejo-bad-sig").await;

    let integration_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO diraigent.forgejo_integration (id, project_id, base_url, webhook_secret, enabled)
         VALUES ($1, $2, 'https://git.example.com', 'real-secret', true)",
    )
    .bind(integration_id)
    .bind(project_id)
    .execute(&app.pool)
    .await
    .unwrap();

    let body = b"{\"workflow_run\":{\"id\":1}}";
    // Sign with wrong secret
    let wrong_sig = compute_signature("wrong-secret", body);
    let resp = app
        .send(webhook_request(
            integration_id,
            "workflow_run",
            body,
            Some(&wrong_sig),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

/// Unparseable JSON body → 400.
#[tokio::test]
async fn webhook_invalid_json_returns_400() {
    let app = require_db!();
    let project_id = app.create_project("forgejo-bad-json").await;

    let secret = "json-test-secret";
    let integration_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO diraigent.forgejo_integration (id, project_id, base_url, webhook_secret, enabled)
         VALUES ($1, $2, 'https://git.example.com', $3, true)",
    )
    .bind(integration_id)
    .bind(project_id)
    .bind(secret)
    .execute(&app.pool)
    .await
    .unwrap();

    let body = b"not valid json at all";
    let signature = compute_signature(secret, body);
    let resp = app
        .send(webhook_request(
            integration_id,
            "workflow_run",
            body,
            Some(&signature),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}

/// Unrecognised event type → 200 (graceful no-op).
#[tokio::test]
async fn webhook_unrecognised_event_returns_200() {
    let app = require_db!();
    let project_id = app.create_project("forgejo-unknown-event").await;

    let secret = "noop-test-secret";
    let integration_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO diraigent.forgejo_integration (id, project_id, base_url, webhook_secret, enabled)
         VALUES ($1, $2, 'https://git.example.com', $3, true)",
    )
    .bind(integration_id)
    .bind(project_id)
    .bind(secret)
    .execute(&app.pool)
    .await
    .unwrap();

    let body = b"{\"some\":\"data\"}";
    let signature = compute_signature(secret, body);
    let resp = app
        .send(webhook_request(
            integration_id,
            "push",
            body,
            Some(&signature),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["status"], "ok");

    // No ci_run should have been created
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM diraigent.ci_run WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count.0, 0, "no ci_run should be created for unknown events");

    app.cleanup().await;
}

/// Non-existent integration ID → 404.
#[tokio::test]
async fn webhook_unknown_integration_returns_404() {
    let app = require_db!();

    let fake_id = Uuid::now_v7();
    let body = b"{}";
    let resp = app
        .send(webhook_request(
            fake_id,
            "workflow_run",
            body,
            Some("deadbeef"),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}
