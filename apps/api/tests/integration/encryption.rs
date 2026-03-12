use crate::harness::*;
use axum::http::StatusCode;

// ── Tenant CRUD ──

#[tokio::test]
async fn tenant_crud() {
    let app = require_db!();

    // Create a tenant
    let resp = app
        .send(post_json(
            "/v1/tenants",
            serde_json::json!({ "name": "Test Org" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create tenant: {}", resp.json);
    let tenant_id = resp.json["id"].as_str().unwrap().to_string();
    assert_eq!(resp.json["name"], "Test Org");
    assert_eq!(resp.json["slug"], "test-org");
    assert_eq!(resp.json["encryption_mode"], "none");

    // Get tenant
    let resp = app.send(get(&format!("/v1/tenants/{tenant_id}"))).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Test Org");

    // Update tenant name
    let resp = app
        .send(put_json(
            &format!("/v1/tenants/{tenant_id}"),
            serde_json::json!({ "name": "Updated Org" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Updated Org");

    // List tenants
    let resp = app.send(get("/v1/tenants")).await;
    assert_eq!(resp.status, StatusCode::OK);
    let tenants = resp.json.as_array().unwrap();
    // Should contain at least default + our new one
    assert!(tenants.len() >= 2);

    // Get tenant by slug
    let resp = app.send(get("/v1/tenants/by-slug/test-org")).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["name"], "Updated Org");

    // Get my tenant (should return the default tenant since dev user is in that one)
    let resp = app.send(get("/v1/tenants/me")).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["slug"], "default");

    // Delete tenant
    let resp = app.send(delete(&format!("/v1/tenants/{tenant_id}"))).await;
    assert!(
        resp.status == StatusCode::OK || resp.status == StatusCode::NO_CONTENT,
        "delete tenant: {} {}",
        resp.status,
        resp.json
    );

    app.cleanup().await;
}

// ── Encryption init and unlock ──

#[tokio::test]
async fn encryption_init_and_unlock() {
    let app = require_db!();

    // Create a fresh tenant for this test
    let resp = app
        .send(post_json(
            "/v1/tenants",
            serde_json::json!({ "name": "Crypto Org" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let tenant_id = resp.json["id"].as_str().unwrap().to_string();

    // Verify it starts with no encryption
    let resp = app
        .send(get(&format!("/v1/tenants/{tenant_id}/encryption/salt")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["encryption_mode"], "none");

    // Initialize encryption with a fake access token
    let access_token = "test-access-token-for-init";
    let resp = app
        .send(post_json(
            &format!("/v1/tenants/{tenant_id}/encryption/init"),
            serde_json::json!({ "access_token": access_token }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "init encryption: {}",
        resp.json
    );
    assert_eq!(resp.json["encryption_mode"], "login_derived");
    assert!(resp.json["salt"].is_string());
    assert!(resp.json["wrapped_dek"].is_string());

    // Verify tenant is now login_derived
    let resp = app
        .send(get(&format!("/v1/tenants/{tenant_id}/encryption/salt")))
        .await;
    assert_eq!(resp.json["encryption_mode"], "login_derived");
    assert!(resp.json["salt"].is_string());

    // Unlock encryption with the same token
    let resp = app
        .send(post_json(
            &format!("/v1/tenants/{tenant_id}/encryption/unlock"),
            serde_json::json!({ "access_token": access_token }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "unlock: {}", resp.json);
    // DEK was cached by init, so should be already_unlocked
    let status = resp.json["status"].as_str().unwrap();
    assert!(
        status == "unlocked" || status == "already_unlocked",
        "unexpected status: {status}"
    );

    // Second init attempt should fail (already initialized)
    let resp = app
        .send(post_json(
            &format!("/v1/tenants/{tenant_id}/encryption/init"),
            serde_json::json!({ "access_token": access_token }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::CONFLICT);

    app.cleanup().await;
}

// ── Encrypted field roundtrip ──

#[tokio::test]
async fn encrypted_field_roundtrip() {
    let app = require_db!();

    // Create a tenant and enable encryption
    let resp = app
        .send(post_json(
            "/v1/tenants",
            serde_json::json!({ "name": "Encrypted Org" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let tenant_id = resp.json["id"].as_str().unwrap().to_string();

    let access_token = "roundtrip-test-token";
    let resp = app
        .send(post_json(
            &format!("/v1/tenants/{tenant_id}/encryption/init"),
            serde_json::json!({ "access_token": access_token }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    // Create a project under this tenant
    let resp = app
        .send(post_json(
            "/v1",
            serde_json::json!({
                "name": "Encrypted Project",
                "tenant_id": tenant_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create project: {}", resp.json);
    let project_id = resp.json["id"].as_str().unwrap().to_string();

    // Create a task with context (should be encrypted at rest)
    let context = serde_json::json!({
        "spec": "Build the encryption feature",
        "files": ["src/crypto.rs"],
        "secret_key": "this-should-be-encrypted",
    });

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks"),
            serde_json::json!({
                "title": "Encryption test task",
                "context": context,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create task: {}", resp.json);
    let task_id = resp.json["id"].as_str().unwrap().to_string();

    // GET the task — CryptoDb should decrypt transparently
    let resp = app.send(get(&format!("/v1/tasks/{task_id}"))).await;
    assert_eq!(resp.status, StatusCode::OK, "get task: {}", resp.json);
    assert_eq!(resp.json["context"]["spec"], "Build the encryption feature");
    assert_eq!(
        resp.json["context"]["secret_key"],
        "this-should-be-encrypted"
    );

    // Verify the raw DB has encrypted data by querying directly
    let row: (serde_json::Value,) =
        sqlx::query_as("SELECT context FROM diraigent.task WHERE id = $1::uuid")
            .bind(&task_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    // The context should be an encrypted string value, not a JSON object
    if let serde_json::Value::String(s) = &row.0 {
        assert!(
            s.starts_with("enc:v1:"),
            "expected encrypted context in DB, got: {}",
            &s[..s.len().min(40)]
        );
    } else {
        panic!("expected encrypted string in DB, got: {:?}", row.0);
    }

    app.cleanup().await;
}

// ── Member key management ──

#[tokio::test]
async fn member_key_management() {
    let app = require_db!();

    // Use the default tenant — the dev user is already an owner
    let resp = app.send(get("/v1/tenants/me")).await;
    assert_eq!(resp.status, StatusCode::OK);
    let tenant_id = resp.json["id"].as_str().unwrap().to_string();

    // List members — should include dev user
    let resp = app
        .send(get(&format!("/v1/tenants/{tenant_id}/members")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let members = resp.json.as_array().unwrap();
    assert!(!members.is_empty(), "should have at least one member");
    let user_id = members[0]["user_id"].as_str().unwrap().to_string();

    // Create a wrapped key
    let resp = app
        .send(post_json(
            &format!("/v1/tenants/{tenant_id}/members/{user_id}/keys"),
            serde_json::json!({
                "key_type": "login_derived",
                "wrapped_dek": "dGVzdC13cmFwcGVkLWRlayBiYXNlNjQ=",
                "kdf_salt": "dGVzdC1zYWx0LWJhc2U2NA==",
                "key_version": 1,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create key: {}", resp.json);
    let key_id = resp.json["id"].as_str().unwrap().to_string();
    assert_eq!(resp.json["key_type"], "login_derived");

    // List wrapped keys
    let resp = app
        .send(get(&format!(
            "/v1/tenants/{tenant_id}/members/{user_id}/keys"
        )))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let keys = resp.json.as_array().unwrap();
    assert!(
        keys.iter().any(|k| k["id"].as_str() == Some(&key_id)),
        "should contain the created key"
    );

    // Delete wrapped key
    let resp = app
        .send(delete(&format!("/v1/tenants/{tenant_id}/keys/{key_id}")))
        .await;
    assert!(
        resp.status == StatusCode::OK || resp.status == StatusCode::NO_CONTENT,
        "delete key: {} {}",
        resp.status,
        resp.json
    );

    // Verify it's gone
    let resp = app
        .send(get(&format!(
            "/v1/tenants/{tenant_id}/members/{user_id}/keys"
        )))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let keys = resp.json.as_array().unwrap();
    assert!(
        !keys.iter().any(|k| k["id"].as_str() == Some(&key_id)),
        "key should be deleted"
    );

    app.cleanup().await;
}
