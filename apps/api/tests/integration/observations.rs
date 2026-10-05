use crate::harness::*;
use axum::http::StatusCode;

#[tokio::test]
async fn promote_observation_starts_backlog() {
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
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create observation: {}",
        resp.json
    );
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "promote observation: {}",
        resp.json
    );

    let task = &resp.json["task"];
    assert_eq!(task["state"].as_str().unwrap(), "backlog");

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
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create observation: {}",
        resp.json
    );
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
    assert_eq!(
        resp.status,
        StatusCode::CONFLICT,
        "second promote: {}",
        resp.json
    );

    app.cleanup().await;
}
