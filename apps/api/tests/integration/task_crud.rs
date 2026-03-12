use crate::harness::*;
use axum::http::StatusCode;

#[tokio::test]
async fn create_and_get_task() {
    let app = require_db!();
    let project_id = app.create_project("crud-test").await;

    let task = app.create_task(project_id, "My first task").await;
    assert_eq!(task["title"].as_str().unwrap(), "My first task");
    assert_eq!(task["state"].as_str().unwrap(), "backlog");
    assert_eq!(task["kind"].as_str().unwrap(), "feature");

    // GET by id
    let task_id = task["id"].as_str().unwrap();
    let resp = app.send(get(&format!("/v1/tasks/{task_id}"))).await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["title"].as_str().unwrap(), "My first task");

    app.cleanup().await;
}

#[tokio::test]
async fn create_task_with_options() {
    let app = require_db!();
    let project_id = app.create_project("crud-opts").await;

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Bug fix",
                "kind": "bug",
                "priority": 5,
                "context": { "spec": "fix the login" },
            }),
        )
        .await;
    assert_eq!(task["kind"].as_str().unwrap(), "bug");
    assert_eq!(task["priority"].as_i64().unwrap(), 5);
    assert_eq!(task["context"]["spec"].as_str().unwrap(), "fix the login");

    app.cleanup().await;
}

#[tokio::test]
async fn list_tasks_for_project() {
    let app = require_db!();
    let project_id = app.create_project("list-test").await;

    app.create_task(project_id, "Task A").await;
    app.create_task(project_id, "Task B").await;

    let resp = app.send(get(&format!("/v1/{project_id}/tasks"))).await;
    assert_eq!(resp.status, StatusCode::OK);
    let data = resp.json["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(resp.json["total"].as_i64().unwrap(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn list_tasks_with_state_filter() {
    let app = require_db!();
    let project_id = app.create_project("filter-test").await;

    let task = app.create_task(project_id, "Will be ready").await;
    let task_id = task["id"].as_str().unwrap();
    app.create_task(project_id, "Still backlog").await;

    // Transition first task to ready
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    // Filter by state=ready
    let resp = app
        .send(get(&format!("/v1/{project_id}/tasks?state=ready")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let data = resp.json["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["title"].as_str().unwrap(), "Will be ready");

    app.cleanup().await;
}

#[tokio::test]
async fn update_task_fields() {
    let app = require_db!();
    let project_id = app.create_project("update-test").await;

    let task = app.create_task(project_id, "Original title").await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({
                "title": "Updated title",
                "priority": 10,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["title"].as_str().unwrap(), "Updated title");
    assert_eq!(resp.json["priority"].as_i64().unwrap(), 10);

    app.cleanup().await;
}

#[tokio::test]
async fn task_updates_crud() {
    let app = require_db!();
    let project_id = app.create_project("updates-test").await;

    let task = app.create_task(project_id, "Task with updates").await;
    let task_id = task["id"].as_str().unwrap();

    // Create a progress update
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/updates"),
            serde_json::json!({
                "content": "Started working on it",
                "kind": "progress",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        resp.json["content"].as_str().unwrap(),
        "Started working on it"
    );

    // List updates
    let resp = app.send(get(&format!("/v1/tasks/{task_id}/updates"))).await;
    assert_eq!(resp.status, StatusCode::OK);
    let updates = resp.json.as_array().unwrap();
    assert_eq!(updates.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn task_comments_crud() {
    let app = require_db!();
    let project_id = app.create_project("comments-test").await;

    let task = app.create_task(project_id, "Task with comments").await;
    let task_id = task["id"].as_str().unwrap();

    // Create comment
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/comments"),
            serde_json::json!({
                "content": "Looks good to me",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["content"].as_str().unwrap(), "Looks good to me");

    // List comments
    let resp = app
        .send(get(&format!("/v1/tasks/{task_id}/comments")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let comments = resp.json.as_array().unwrap();
    assert_eq!(comments.len(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn task_dependencies() {
    let app = require_db!();
    let project_id = app.create_project("deps-test").await;

    let task_a = app.create_task(project_id, "Blocker").await;
    let task_b = app.create_task(project_id, "Blocked").await;
    let a_id = task_a["id"].as_str().unwrap();
    let b_id = task_b["id"].as_str().unwrap();

    // B depends on A
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{b_id}/dependencies"),
            serde_json::json!({ "depends_on": a_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    // Transition both to ready
    app.send(post_json(
        &format!("/v1/tasks/{a_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{b_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;

    // B should NOT appear in ready tasks (A not done)
    let resp = app
        .send(get(&format!("/v1/{project_id}/tasks/ready")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let ready = resp.json.as_array().unwrap();
    let ids: Vec<&str> = ready.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&a_id));
    assert!(!ids.contains(&b_id));

    // Remove dependency
    let resp = app
        .send(delete(&format!("/v1/tasks/{b_id}/dependencies/{a_id}")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    // Now B should appear in ready tasks
    let resp = app
        .send(get(&format!("/v1/{project_id}/tasks/ready")))
        .await;
    let ready = resp.json.as_array().unwrap();
    let ids: Vec<&str> = ready.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&b_id));

    app.cleanup().await;
}

#[tokio::test]
async fn get_nonexistent_task_returns_404() {
    let app = require_db!();
    let fake_id = uuid::Uuid::now_v7();

    let resp = app.send(get(&format!("/v1/tasks/{fake_id}"))).await;
    assert_eq!(resp.status, StatusCode::NOT_FOUND);

    app.cleanup().await;
}

#[tokio::test]
async fn create_task_with_invalid_kind_returns_400() {
    let app = require_db!();
    let project_id = app.create_project("invalid-kind").await;

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks"),
            serde_json::json!({
                "title": "Bad kind",
                "kind": "nonexistent",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    app.cleanup().await;
}
