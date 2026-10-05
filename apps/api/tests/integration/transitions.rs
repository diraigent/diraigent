use crate::harness::*;
use axum::http::StatusCode;
use uuid::Uuid;

#[tokio::test]
async fn backlog_to_ready() {
    let app = require_db!();
    let project_id = app.create_project("trans-1").await;

    let task = app.create_task(project_id, "Transition test").await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["state"].as_str().unwrap(), "backlog");

    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");

    app.cleanup().await;
}

#[tokio::test]
async fn ready_to_working_via_claim() {
    let app = require_db!();
    let project_id = app.create_project("trans-2").await;
    let agent_id = app.create_agent("claimer").await;

    let task = app.create_task(project_id, "Claim me").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;

    // claim (ready → working for tasks without playbook)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "working");
    assert_eq!(
        resp.json["assigned_agent_id"].as_str().unwrap(),
        agent_id.to_string()
    );

    app.cleanup().await;
}

#[tokio::test]
async fn working_to_done() {
    let app = require_db!();
    let project_id = app.create_project("trans-3").await;
    let agent_id = app.create_agent("worker").await;

    let task = app.create_task(project_id, "Finish me").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready → working
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;

    // working → done
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "done");
    assert!(resp.json["completed_at"].as_str().is_some());

    app.cleanup().await;
}

#[tokio::test]
async fn release_returns_to_ready() {
    let app = require_db!();
    let project_id = app.create_project("trans-4").await;
    let agent_id = app.create_agent("releaser").await;

    let task = app.create_task(project_id, "Release me").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready → working
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;

    // release (working → ready)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/release"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert!(resp.json["assigned_agent_id"].is_null());

    app.cleanup().await;
}

#[tokio::test]
async fn cancel_from_backlog() {
    let app = require_db!();
    let project_id = app.create_project("trans-5").await;

    let task = app.create_task(project_id, "Cancel me").await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "cancelled" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "cancelled");

    app.cleanup().await;
}

#[tokio::test]
async fn reopen_cancelled_task() {
    let app = require_db!();
    let project_id = app.create_project("trans-6").await;

    let task = app.create_task(project_id, "Reopen me").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → cancelled → backlog
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "cancelled" }),
    ))
    .await;

    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "backlog" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "backlog");

    app.cleanup().await;
}

#[tokio::test]
async fn invalid_backlog_to_done() {
    let app = require_db!();
    let project_id = app.create_project("trans-inv-1").await;

    let task = app.create_task(project_id, "Cannot skip").await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn invalid_ready_to_done_directly() {
    let app = require_db!();
    let project_id = app.create_project("trans-inv-2").await;

    let task = app.create_task(project_id, "Cannot skip").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;

    // ready → done should fail (done is a lifecycle state, not a step name)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn claim_non_ready_task_fails() {
    let app = require_db!();
    let project_id = app.create_project("trans-inv-3").await;
    let agent_id = app.create_agent("eager").await;

    let task = app.create_task(project_id, "Not ready yet").await;
    let task_id = task["id"].as_str().unwrap();

    // Try to claim a backlog task
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn release_lifecycle_state_fails() {
    let app = require_db!();
    let project_id = app.create_project("trans-inv-4").await;

    let task = app
        .create_task(project_id, "Cannot release from backlog")
        .await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/release"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn done_to_ready_reopen() {
    let app = require_db!();
    let project_id = app.create_project("trans-advance").await;
    let agent_id = app.create_agent("pipeliner").await;

    let task = app.create_task(project_id, "Pipeline task").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready → working → done (no playbook, so done is terminal)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "done" }),
    ))
    .await;

    // done → ready (manual reopen)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");

    app.cleanup().await;
}

#[tokio::test]
async fn done_to_human_review() {
    let app = require_db!();
    let project_id = app.create_project("trans-hr-1").await;
    let agent_id = app.create_agent("reviewer").await;

    let task = app.create_task(project_id, "Needs human review").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready → working → done
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "done" }),
    ))
    .await;

    // done → human_review
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "human_review" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "human_review");

    app.cleanup().await;
}

#[tokio::test]
async fn human_review_to_done() {
    let app = require_db!();
    let project_id = app.create_project("trans-hr-2").await;
    let agent_id = app.create_agent("approver").await;

    let task = app.create_task(project_id, "Approve after review").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready → working → done → human_review
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "done" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "human_review" }),
    ))
    .await;

    // human_review → done (approved)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "done");

    app.cleanup().await;
}

#[tokio::test]
async fn human_review_to_ready() {
    let app = require_db!();
    let project_id = app.create_project("trans-hr-3").await;
    let agent_id = app.create_agent("reworker").await;

    let task = app.create_task(project_id, "Rework after review").await;
    let task_id = task["id"].as_str().unwrap();

    // backlog → ready → working → done → human_review
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "done" }),
    ))
    .await;
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "human_review" }),
    ))
    .await;

    // human_review → ready (rework needed)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_transition_all_success_returns_200() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-ok").await;

    // Create two tasks in backlog
    let t1 = app.create_task(project_id, "Bulk t1").await;
    let t2 = app.create_task(project_id, "Bulk t2").await;
    let t1_id: uuid::Uuid = t1["id"].as_str().unwrap().parse().unwrap();
    let t2_id: uuid::Uuid = t2["id"].as_str().unwrap().parse().unwrap();

    // Bulk transition backlog → ready
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [t1_id, t2_id],
                "state": "ready"
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 2);
    assert!(failed.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_transition_partial_failure_returns_207() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-partial").await;

    // Create one valid task in backlog
    let t1 = app.create_task(project_id, "Bulk partial t1").await;
    let t1_id: uuid::Uuid = t1["id"].as_str().unwrap().parse().unwrap();

    // Use a random UUID that doesn't exist
    let fake_id = uuid::Uuid::new_v4();

    // Bulk transition: t1 should succeed (backlog→ready), fake should fail
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [t1_id, fake_id],
                "state": "ready"
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::MULTI_STATUS);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 1);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), fake_id.to_string());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_transition_all_fail_returns_400() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-fail").await;

    let fake1 = uuid::Uuid::new_v4();
    let fake2 = uuid::Uuid::new_v4();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [fake1, fake2],
                "state": "ready"
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_transition_empty_task_ids_returns_200() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-empty").await;

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [],
                "state": "ready"
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert!(failed.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delete_all_success_returns_200() {
    let app = require_db!();
    let project_id = app.create_project("bulk-del-ok2").await;

    let t1 = app.create_task(project_id, "Del t1").await;
    let t2 = app.create_task(project_id, "Del t2").await;
    let t1_id: uuid::Uuid = t1["id"].as_str().unwrap().parse().unwrap();
    let t2_id: uuid::Uuid = t2["id"].as_str().unwrap().parse().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delete"),
            serde_json::json!({ "task_ids": [t1_id, t2_id] }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 2);
    assert!(failed.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delete_all_fail_returns_400() {
    let app = require_db!();
    let project_id = app.create_project("bulk-del-fail").await;

    let fake1 = uuid::Uuid::new_v4();
    let fake2 = uuid::Uuid::new_v4();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delete"),
            serde_json::json!({ "task_ids": [fake1, fake2] }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delete_empty_returns_200() {
    let app = require_db!();
    let project_id = app.create_project("bulk-del-empty2").await;

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delete"),
            serde_json::json!({ "task_ids": [] }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert!(failed.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delete_cross_project_task_fails() {
    let app = require_db!();
    let project_a = app.create_project("bulk-del-proj-a").await;
    let project_b = app.create_project("bulk-del-proj-b").await;

    // Create a task in project B
    let task_b = app.create_task(project_b, "Wrong project del").await;
    let task_b_id = task_b["id"].as_str().unwrap();

    // Try to bulk-delete it via project A's endpoint
    let resp = app
        .send(post_json(
            &format!("/v1/{project_a}/tasks/bulk/delete"),
            serde_json::json!({ "task_ids": [task_b_id] }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), task_b_id);
    assert!(
        failed[0]["error"]
            .as_str()
            .unwrap()
            .contains("does not belong to this project")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delete_partial_failure_returns_207() {
    let app = require_db!();
    let project_id = app.create_project("bulk-del-partial").await;

    let t1 = app.create_task(project_id, "Del partial ok").await;
    let t1_id: uuid::Uuid = t1["id"].as_str().unwrap().parse().unwrap();
    let fake_id = uuid::Uuid::new_v4();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delete"),
            serde_json::json!({ "task_ids": [t1_id, fake_id] }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::MULTI_STATUS);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 1);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), fake_id.to_string());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delegate_all_success() {
    let app = require_db!();
    let project_id = app.create_project("bulk-deleg-ok").await;
    let agent_id = app.create_agent("delegatee").await;

    let t1 = app.create_task(project_id, "Delegate me 1").await;
    let t2 = app.create_task(project_id, "Delegate me 2").await;
    let t1_id = t1["id"].as_str().unwrap();
    let t2_id = t2["id"].as_str().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delegate"),
            serde_json::json!({
                "task_ids": [t1_id, t2_id],
                "agent_id": agent_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 2);
    assert!(failed.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delegate_partial_failure() {
    let app = require_db!();
    let project_id = app.create_project("bulk-deleg-partial").await;
    let agent_id = app.create_agent("delegatee-p").await;

    let t1 = app.create_task(project_id, "Delegate partial").await;
    let t1_id = t1["id"].as_str().unwrap();
    let fake_id = Uuid::now_v7();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delegate"),
            serde_json::json!({
                "task_ids": [t1_id, fake_id.to_string()],
                "agent_id": agent_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 1);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), fake_id.to_string());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delegate_all_fail() {
    let app = require_db!();
    let project_id = app.create_project("bulk-deleg-allfail").await;
    let agent_id = app.create_agent("delegatee-f").await;

    let fake1 = Uuid::now_v7();
    let fake2 = Uuid::now_v7();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delegate"),
            serde_json::json!({
                "task_ids": [fake1.to_string(), fake2.to_string()],
                "agent_id": agent_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delegate_empty_task_ids() {
    let app = require_db!();
    let project_id = app.create_project("bulk-deleg-empty").await;
    let agent_id = app.create_agent("delegatee-e").await;

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delegate"),
            serde_json::json!({
                "task_ids": [],
                "agent_id": agent_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert!(failed.is_empty());

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delegate_cross_project_task_fails() {
    let app = require_db!();
    let project_a = app.create_project("bulk-deleg-proj-a").await;
    let project_b = app.create_project("bulk-deleg-proj-b").await;
    let agent_id = app.create_agent("delegatee-x").await;

    // Create a task in project B
    let task_b = app.create_task(project_b, "Wrong project task").await;
    let task_b_id = task_b["id"].as_str().unwrap();

    // Try to bulk-delegate it via project A's endpoint
    let resp = app
        .send(post_json(
            &format!("/v1/{project_a}/tasks/bulk/delegate"),
            serde_json::json!({
                "task_ids": [task_b_id],
                "agent_id": agent_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), task_b_id);
    assert!(
        failed[0]["error"]
            .as_str()
            .unwrap()
            .contains("does not belong to this project")
    );

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_delegate_nonexistent_agent_fails() {
    let app = require_db!();
    let project_id = app.create_project("bulk-deleg-noagent").await;

    let t1 = app.create_task(project_id, "Delegate to ghost").await;
    let t1_id = t1["id"].as_str().unwrap();
    let fake_agent_id = Uuid::now_v7();

    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/delegate"),
            serde_json::json!({
                "task_ids": [t1_id],
                "agent_id": fake_agent_id.to_string(),
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), t1_id);
    let error_msg = failed[0]["error"].as_str().unwrap();
    assert!(!error_msg.is_empty(), "error message should be non-empty");

    app.cleanup().await;
}

// This mirrors the same gap documented for transition_task (task #64).

// ── bulk_transition: file lock release on pipeline advancement ──

#[tokio::test]
async fn bulk_transition_cross_project_task_fails() {
    let app = require_db!();
    let project_a = app.create_project("bulk-trans-proj-a").await;
    let project_b = app.create_project("bulk-trans-proj-b").await;

    // Create a task in project B
    let task_b = app.create_task(project_b, "Wrong project task").await;
    let task_b_id = task_b["id"].as_str().unwrap();

    // Move task to ready so a transition to "cancelled" is valid
    app.send(post_json(
        &format!("/v1/tasks/{task_b_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;

    // Try to bulk-transition it via project A's endpoint
    let resp = app
        .send(post_json(
            &format!("/v1/{project_a}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [task_b_id],
                "state": "cancelled",
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(succeeded.is_empty());
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["task_id"].as_str().unwrap(), task_b_id);
    assert!(
        failed[0]["error"]
            .as_str()
            .unwrap()
            .contains("does not belong to this project")
    );

    app.cleanup().await;
}
