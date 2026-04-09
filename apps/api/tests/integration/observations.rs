use crate::harness::*;
use axum::http::StatusCode;

#[tokio::test]
async fn promote_observation_inherits_default_playbook_name() {
    let app = require_db!();
    let project_id = app.create_project("obs-promote-default").await;

    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}"),
            serde_json::json!({ "default_playbook_name": "standard" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update project: {}", resp.json);

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/observations"),
            serde_json::json!({
                "title": "Memory leak in task processor",
                "kind": "risk",
                "severity": "high",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create observation: {}", resp.json);
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "promote observation: {}", resp.json);

    let task = &resp.json["task"];
    let observation = &resp.json["observation"];

    assert_eq!(task["playbook_name"].as_str().unwrap(), "standard");
    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);
    assert_eq!(observation["status"].as_str().unwrap(), "acted_on");

    app.cleanup().await;
}

#[tokio::test]
async fn promote_observation_with_explicit_playbook_name_override() {
    let app = require_db!();
    let project_id = app.create_project("obs-promote-override").await;

    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}"),
            serde_json::json!({ "default_playbook_name": "standard" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update project: {}", resp.json);

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/observations"),
            serde_json::json!({
                "title": "Critical bug in production",
                "kind": "risk",
                "severity": "high",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create observation: {}", resp.json);
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({ "playbook_name": "research" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "promote with override: {}", resp.json);

    let task = &resp.json["task"];
    assert_eq!(task["playbook_name"].as_str().unwrap(), "research");
    assert_eq!(task["state"].as_str().unwrap(), "ready");

    app.cleanup().await;
}

#[tokio::test]
async fn promote_observation_without_any_playbook_starts_backlog() {
    let app = require_db!();
    let project_id = app.create_project("obs-promote-none").await;

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/observations"),
            serde_json::json!({
                "title": "General cleanup candidate",
                "kind": "improvement",
                "severity": "low",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create observation: {}", resp.json);
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "promote observation: {}", resp.json);

    let task = &resp.json["task"];
    assert_eq!(task["state"].as_str().unwrap(), "backlog");
    assert!(task["playbook_name"].is_null());
    assert!(task["playbook_step"].is_null());

    app.cleanup().await;
}

#[tokio::test]
async fn promote_observation_twice_is_conflict() {
    let app = require_db!();
    let project_id = app.create_project("obs-double-promote").await;

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/observations"),
            serde_json::json!({
                "title": "Already promoted",
                "kind": "insight",
                "severity": "info",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create observation: {}", resp.json);
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "first promote: {}", resp.json);

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::CONFLICT, "second promote: {}", resp.json);

    app.cleanup().await;
}
