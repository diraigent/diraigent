use crate::harness::*;
use axum::http::StatusCode;

// ── list / create ──────────────────────────────────────────────────────────

#[tokio::test]
async fn list_packages_returns_builtins() {
    let app = require_db!();

    let resp = app.send(get("/v1/packages")).await;
    assert_eq!(resp.status, StatusCode::OK);

    let pkgs = resp.json.as_array().unwrap();
    // Two built-in packages are seeded by migration 021
    assert!(
        pkgs.len() >= 2,
        "expected at least 2 built-in packages, got {}",
        pkgs.len()
    );
    let slugs: Vec<&str> = pkgs.iter().filter_map(|p| p["slug"].as_str()).collect();
    assert!(slugs.contains(&"software-dev"), "software-dev missing");
    assert!(slugs.contains(&"researcher"), "researcher missing");

    app.cleanup().await;
}

#[tokio::test]
async fn create_and_get_package() {
    let app = require_db!();

    let resp = app
        .send(post_json(
            "/v1/packages",
            serde_json::json!({
                "slug": "test-pkg",
                "name": "Test Package",
                "allowed_task_kinds": ["feature", "bug"],
                "allowed_knowledge_categories": ["pattern"],
                "allowed_observation_kinds": ["insight"],
                "allowed_event_kinds": ["custom"],
                "allowed_integration_kinds": ["custom"]
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create: {}", resp.json);
    let id = resp.id();
    assert_eq!(resp.json["slug"].as_str().unwrap(), "test-pkg");
    assert_eq!(resp.json["is_builtin"].as_bool().unwrap(), false);

    // GET by id
    let resp2 = app.send(get(&format!("/v1/packages/{id}"))).await;
    assert_eq!(resp2.status, StatusCode::OK);
    assert_eq!(resp2.json["name"].as_str().unwrap(), "Test Package");

    app.cleanup().await;
}

// ── update: non-builtin slug rename is allowed ─────────────────────────────

#[tokio::test]
async fn update_package_slug_allowed_for_custom() {
    let app = require_db!();

    let create = app
        .send(post_json(
            "/v1/packages",
            serde_json::json!({
                "slug": "rename-me",
                "name": "Original"
            }),
        ))
        .await;
    assert_eq!(create.status, StatusCode::OK);
    let id = create.id();

    let resp = app
        .send(put_json(
            &format!("/v1/packages/{id}"),
            serde_json::json!({ "slug": "renamed" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update: {}", resp.json);
    assert_eq!(resp.json["slug"].as_str().unwrap(), "renamed");

    app.cleanup().await;
}

// ── business rule: cannot rename built-in slug ─────────────────────────────

#[tokio::test]
async fn update_builtin_slug_is_rejected() {
    let app = require_db!();

    // Find the software-dev built-in package
    let list = app.send(get("/v1/packages")).await;
    assert_eq!(list.status, StatusCode::OK);
    let pkgs = list.json.as_array().unwrap();
    let builtin = pkgs
        .iter()
        .find(|p| p["slug"] == "software-dev")
        .expect("software-dev not found");
    let id = builtin["id"].as_str().unwrap();

    let resp = app
        .send(put_json(
            &format!("/v1/packages/{id}"),
            serde_json::json!({ "slug": "hacked-slug" }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::CONFLICT,
        "expected 409 Conflict, got {}: {}",
        resp.status,
        resp.json
    );

    app.cleanup().await;
}

// ── business rule: cannot delete built-in package ─────────────────────────

#[tokio::test]
async fn delete_builtin_package_is_rejected() {
    let app = require_db!();

    let list = app.send(get("/v1/packages")).await;
    let pkgs = list.json.as_array().unwrap();
    let builtin = pkgs
        .iter()
        .find(|p| p["slug"] == "software-dev")
        .expect("software-dev not found");
    let id = builtin["id"].as_str().unwrap();

    let resp = app.send(delete(&format!("/v1/packages/{id}"))).await;
    assert_eq!(
        resp.status,
        StatusCode::CONFLICT,
        "expected 409 Conflict, got {}: {}",
        resp.status,
        resp.json
    );

    app.cleanup().await;
}

// ── business rule: non-builtin packages can be deleted ────────────────────

#[tokio::test]
async fn delete_custom_package_succeeds() {
    let app = require_db!();

    let create = app
        .send(post_json(
            "/v1/packages",
            serde_json::json!({ "slug": "delete-me", "name": "Temp" }),
        ))
        .await;
    assert_eq!(create.status, StatusCode::OK);
    let id = create.id();

    let resp = app.send(delete(&format!("/v1/packages/{id}"))).await;
    assert_eq!(resp.status, StatusCode::NO_CONTENT, "{}", resp.json);

    // confirm it's gone
    let get_resp = app.send(get(&format!("/v1/packages/{id}"))).await;
    assert_eq!(get_resp.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

// ── get_package_for_project ────────────────────────────────────────────────

#[tokio::test]
async fn project_resolves_package() {
    let app = require_db!();

    // Create a project — it picks up the default software-dev package
    let project_id = app.create_project("pkg-resolve").await;

    // The project response should embed package info
    let resp = app.send(get(&format!("/v1/{project_id}"))).await;
    assert_eq!(resp.status, StatusCode::OK, "{}", resp.json);
    let package = &resp.json["package"];
    assert!(
        !package.is_null(),
        "expected package to be set on project, got null"
    );
    assert_eq!(package["slug"].as_str().unwrap(), "software-dev");

    app.cleanup().await;
}
