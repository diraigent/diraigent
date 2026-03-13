use crate::harness::*;
use axum::http::StatusCode;
use uuid::Uuid;

/// Helper: insert a global step template (tenant_id = NULL) directly into the DB.
async fn insert_global_step_template(app: &TestApp) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO diraigent.step_template
         (id, tenant_id, name, description, allowed_tools, model, budget,
          context_level, tags, metadata, created_by)
         VALUES ($1, NULL, 'implement', 'Write code', 'full', 'claude-opus-4-6', 12.0,
                 'full', '{\"default\"}', '{}'::jsonb, '00000000-0000-0000-0000-000000000000')",
    )
    .bind(id)
    .execute(&app.pool)
    .await
    .expect("insert global step template");
    id
}

/// CRUD happy path: create, get, list, update, delete.
#[tokio::test]
async fn step_template_crud() {
    let app = require_db!();
    let project_id = app.create_project("st-crud").await;

    // Create
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/step-templates"),
            serde_json::json!({
                "name": "review",
                "description": "Review the implementation",
                "allowed_tools": "readonly",
                "model": "claude-sonnet-4-6",
                "budget": 5.0,
                "context_level": "minimal",
                "tags": ["review"],
                "metadata": {"auto_approve": false},
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create: {}", resp.json);
    let id = resp.json["id"].as_str().unwrap().to_string();
    assert_eq!(resp.json["name"].as_str().unwrap(), "review");
    assert_eq!(
        resp.json["description"].as_str().unwrap(),
        "Review the implementation"
    );
    assert_eq!(resp.json["allowed_tools"].as_str().unwrap(), "readonly");
    assert!(!resp.json["tenant_id"].is_null(), "should be tenant-owned");

    // Get
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates/{id}")))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "get: {}", resp.json);
    assert_eq!(resp.json["name"].as_str().unwrap(), "review");

    // List
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates")))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "list: {}", resp.json);
    let templates = resp.json.as_array().unwrap();
    assert!(
        templates.iter().any(|t| t["id"].as_str() == Some(&id)),
        "listed templates should include the created one"
    );

    // Update
    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}/step-templates/{id}"),
            serde_json::json!({ "description": "Updated review description" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update: {}", resp.json);
    assert_eq!(
        resp.json["description"].as_str().unwrap(),
        "Updated review description"
    );

    // Delete
    let resp = app
        .send(delete(&format!("/v1/{project_id}/step-templates/{id}")))
        .await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT, "delete: {}", resp.json);

    // Verify deleted
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates/{id}")))
        .await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

/// Global templates visible in list but cannot be updated or deleted.
#[tokio::test]
async fn global_step_template_immutable() {
    let app = require_db!();
    let project_id = app.create_project("st-global").await;
    let global_id = insert_global_step_template(&app).await;

    // Global template should appear in list
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let templates = resp.json.as_array().unwrap();
    assert!(
        templates
            .iter()
            .any(|t| t["id"].as_str() == Some(&global_id.to_string())),
        "global template should be visible in list"
    );

    // Global template should be readable
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates/{global_id}")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert!(resp.json["tenant_id"].is_null());

    // Update should be forbidden
    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}/step-templates/{global_id}"),
            serde_json::json!({ "description": "Hacked" }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::FORBIDDEN,
        "update global: {}",
        resp.json
    );

    // Delete should be forbidden
    let resp = app
        .send(delete(&format!(
            "/v1/{project_id}/step-templates/{global_id}"
        )))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::FORBIDDEN,
        "delete global: {}",
        resp.json
    );

    app.cleanup().await;
}

/// Fork creates a tenant-owned copy of a template.
#[tokio::test]
async fn fork_creates_tenant_copy() {
    let app = require_db!();
    let project_id = app.create_project("st-fork").await;
    let global_id = insert_global_step_template(&app).await;

    // Fork the global template
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/step-templates/{global_id}/fork"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "fork: {}", resp.json);

    // The fork should be a new record
    let forked_id = resp.json["id"].as_str().unwrap();
    assert_ne!(forked_id, global_id.to_string(), "fork must have a new id");

    // The fork must be tenant-owned
    assert!(
        !resp.json["tenant_id"].is_null(),
        "fork must have a tenant_id"
    );

    // The fork should have the same content as the original
    assert_eq!(resp.json["name"].as_str().unwrap(), "implement");
    assert_eq!(resp.json["description"].as_str().unwrap(), "Write code");
    assert_eq!(resp.json["allowed_tools"].as_str().unwrap(), "full");

    // The forked copy should be mutable
    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}/step-templates/{forked_id}"),
            serde_json::json!({ "description": "My custom implement step" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update fork: {}", resp.json);
    assert_eq!(
        resp.json["description"].as_str().unwrap(),
        "My custom implement step"
    );

    app.cleanup().await;
}

/// Cross-tenant isolation: cannot see, update, or delete another tenant's templates.
#[tokio::test]
async fn cross_tenant_isolation() {
    let app = require_db!();
    let project_id = app.create_project("st-isolation").await;

    // Create a template in the current tenant
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/step-templates"),
            serde_json::json!({
                "name": "secret-step",
                "description": "A secret step",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let template_id = resp.json["id"].as_str().unwrap().to_string();

    // Simulate another tenant by inserting a template with a different tenant_id
    let other_tenant_id = Uuid::new_v4();
    let other_template_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO diraigent.step_template
         (id, tenant_id, name, description, tags, metadata, created_by)
         VALUES ($1, $2, 'other-step', 'Other tenant step', '{}', '{}'::jsonb, '00000000-0000-0000-0000-000000000000')",
    )
    .bind(other_template_id)
    .bind(other_tenant_id)
    .execute(&app.pool)
    .await
    .expect("insert other tenant template");

    // List should NOT include the other tenant's template
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let templates = resp.json.as_array().unwrap();
    assert!(
        !templates
            .iter()
            .any(|t| t["id"].as_str() == Some(&other_template_id.to_string())),
        "other tenant's template should NOT appear in list"
    );
    // Our template should still be there
    assert!(
        templates
            .iter()
            .any(|t| t["id"].as_str() == Some(&template_id)),
        "our template should appear in list"
    );

    app.cleanup().await;
}

/// Tag filter works on list endpoint.
#[tokio::test]
async fn list_filters_by_tag() {
    let app = require_db!();
    let project_id = app.create_project("st-tags").await;

    // Create two templates with different tags
    app.send(post_json(
        &format!("/v1/{project_id}/step-templates"),
        serde_json::json!({
            "name": "implement",
            "description": "Implement the feature",
            "tags": ["code"],
        }),
    ))
    .await;

    app.send(post_json(
        &format!("/v1/{project_id}/step-templates"),
        serde_json::json!({
            "name": "review",
            "description": "Review the implementation",
            "tags": ["review"],
        }),
    ))
    .await;

    // Filter by tag
    let resp = app
        .send(get(&format!("/v1/{project_id}/step-templates?tag=review")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let templates = resp.json.as_array().unwrap();
    assert_eq!(
        templates.len(),
        1,
        "should find exactly one with 'review' tag"
    );
    assert_eq!(templates[0]["name"].as_str().unwrap(), "review");

    app.cleanup().await;
}
