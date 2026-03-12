use crate::harness::*;
use axum::http::StatusCode;

/// Promote an observation: the created task gets the project's default playbook
/// and starts in "ready" state (matching the playbook's initial_state).
#[tokio::test]
async fn promote_observation_inherits_default_playbook() {
    let app = require_db!();
    let project_id = app.create_project("obs-promote-pb").await;

    // Create a playbook with initial_state = "ready"
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Default Pipeline",
                "steps": [{"name": "implement"}, {"name": "review"}],
                "initial_state": "ready",
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create playbook: {}",
        resp.json
    );
    let playbook_id = resp.json["id"].as_str().unwrap().to_string();

    // Set the project's default_playbook_id
    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}"),
            serde_json::json!({ "default_playbook_id": playbook_id }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "set default playbook: {}",
        resp.json
    );

    // Create an observation
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
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create observation: {}",
        resp.json
    );
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    // Promote the observation → should create a task with the default playbook
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
    let observation = &resp.json["observation"];

    // Task should have the project's default playbook assigned
    assert_eq!(
        task["playbook_id"].as_str().unwrap(),
        playbook_id,
        "task should inherit the project's default playbook"
    );

    // Task should start in "ready" state (matching playbook's initial_state)
    assert_eq!(
        task["state"].as_str().unwrap(),
        "ready",
        "task should start in ready state per the playbook's initial_state"
    );

    // Task title should default to the observation's title
    assert_eq!(
        task["title"].as_str().unwrap(),
        "Memory leak in task processor"
    );

    // Observation should be marked acted_on and reference the task
    assert_eq!(observation["status"].as_str().unwrap(), "acted_on");
    assert_eq!(
        observation["resolved_task_id"].as_str().unwrap(),
        task["id"].as_str().unwrap()
    );

    app.cleanup().await;
}

/// Promote with a playbook whose initial_state is "backlog" — task starts in backlog.
#[tokio::test]
async fn promote_observation_respects_backlog_initial_state() {
    let app = require_db!();
    let project_id = app.create_project("obs-promote-backlog").await;

    // Create a playbook with initial_state = "backlog"
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Backlog Pipeline",
                "steps": [{"name": "implement"}],
                "initial_state": "backlog",
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create playbook: {}",
        resp.json
    );
    let playbook_id = resp.json["id"].as_str().unwrap().to_string();

    // Set as project default
    app.send(put_json(
        &format!("/v1/{project_id}"),
        serde_json::json!({ "default_playbook_id": playbook_id }),
    ))
    .await;

    // Create and promote an observation
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/observations"),
            serde_json::json!({
                "title": "Refactor auth module",
                "kind": "improvement",
                "severity": "low",
            }),
        ))
        .await;
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "promote: {}", resp.json);

    let task = &resp.json["task"];
    assert_eq!(
        task["state"].as_str().unwrap(),
        "backlog",
        "task should start in backlog per the playbook's initial_state"
    );
    assert_eq!(task["playbook_id"].as_str().unwrap(), playbook_id);

    app.cleanup().await;
}

/// Promote with explicit playbook_id override — overrides the project default.
#[tokio::test]
async fn promote_observation_with_explicit_playbook_override() {
    let app = require_db!();
    let project_id = app.create_project("obs-promote-override").await;

    // Default playbook
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Default",
                "steps": [{"name": "implement"}],
                "initial_state": "backlog",
            }),
        ))
        .await;
    let default_playbook_id = resp.json["id"].as_str().unwrap().to_string();

    // Override playbook
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Fast Track",
                "steps": [{"name": "hotfix"}],
                "initial_state": "ready",
            }),
        ))
        .await;
    let override_playbook_id = resp.json["id"].as_str().unwrap().to_string();

    // Set default playbook on project
    app.send(put_json(
        &format!("/v1/{project_id}"),
        serde_json::json!({ "default_playbook_id": default_playbook_id }),
    ))
    .await;

    // Create observation
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
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    // Promote with explicit playbook override
    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({ "playbook_id": override_playbook_id }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "promote with override: {}",
        resp.json
    );

    let task = &resp.json["task"];
    assert_eq!(
        task["playbook_id"].as_str().unwrap(),
        override_playbook_id,
        "explicit playbook_id should override the project default"
    );
    assert_eq!(
        task["state"].as_str().unwrap(),
        "ready",
        "override playbook's initial_state (ready) should be respected"
    );

    app.cleanup().await;
}

/// Promoting an already-promoted observation returns 409 Conflict.
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
    let obs_id = resp.json["id"].as_str().unwrap().to_string();

    // First promote: succeeds
    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "first promote: {}", resp.json);

    // Second promote: conflict
    let resp = app
        .send(post_json(
            &format!("/v1/observations/{obs_id}/promote"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::CONFLICT,
        "second promote should be 409: {}",
        resp.json
    );

    app.cleanup().await;
}
