use crate::harness::*;
use axum::http::StatusCode;

#[tokio::test]
async fn task_with_playbook_name_starts_ready() {
    let app = require_db!();
    let project_id = app.create_project("pb-name-ready").await;

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Pipeline task",
                "playbook_name": "standard",
            }),
        )
        .await;

    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);
    assert_eq!(task["playbook_name"].as_str().unwrap(), "standard");

    app.cleanup().await;
}

#[tokio::test]
async fn task_without_playbook_starts_backlog() {
    let app = require_db!();
    let project_id = app.create_project("pb-none-backlog").await;

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "No pipeline task",
            }),
        )
        .await;

    assert_eq!(task["state"].as_str().unwrap(), "backlog");
    assert!(task["playbook_name"].is_null());
    assert!(task["playbook_step"].is_null());

    app.cleanup().await;
}

#[tokio::test]
async fn project_default_playbook_name_is_used_for_new_tasks() {
    let app = require_db!();
    let project_id = app.create_project("pb-default-project").await;

    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}"),
            serde_json::json!({ "default_playbook_name": "standard" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update project: {}", resp.json);
    assert_eq!(
        resp.json["default_playbook_name"].as_str().unwrap(),
        "standard"
    );

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Inherited playbook task",
            }),
        )
        .await;

    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_name"].as_str().unwrap(), "standard");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

    app.cleanup().await;
}

#[tokio::test]
async fn explicit_playbook_name_overrides_project_default() {
    let app = require_db!();
    let project_id = app.create_project("pb-override-project").await;

    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}"),
            serde_json::json!({ "default_playbook_name": "standard" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update project: {}", resp.json);

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Override playbook task",
                "playbook_name": "research",
            }),
        )
        .await;

    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_name"].as_str().unwrap(), "research");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

    app.cleanup().await;
}
