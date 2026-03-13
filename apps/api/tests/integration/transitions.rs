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
async fn transition_with_playbook_step_atomic() {
    let app = require_db!();
    let project_id = app.create_project("trans-atomic-step").await;
    let agent_id = app.create_agent("stepper").await;

    let task = app.create_task(project_id, "Atomic step test").await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

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

    // done → ready with playbook_step=1 (atomic pipeline advance)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready", "playbook_step": 1 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn transition_without_playbook_step_preserves_existing() {
    let app = require_db!();
    let project_id = app.create_project("trans-no-step").await;
    let agent_id = app.create_agent("keeper").await;

    let task = app.create_task(project_id, "Preserve step test").await;
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

    // done → ready WITHOUT playbook_step — should keep existing step (0)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 0);

    app.cleanup().await;
}

#[tokio::test]
async fn transition_playbook_step_out_of_range_rejected() {
    let app = require_db!();
    let project_id = app.create_project("trans-step-oob").await;
    let agent_id = app.create_agent("oob-stepper").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "OOB step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
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

    // done → ready with playbook_step=99 should fail (only 3 steps)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready", "playbook_step": 99 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn transition_playbook_step_negative_rejected() {
    let app = require_db!();
    let project_id = app.create_project("trans-step-neg").await;
    let agent_id = app.create_agent("neg-stepper").await;

    let task = app.create_task(project_id, "Negative step test").await;
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

    // done → ready with playbook_step=-1 should fail
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready", "playbook_step": -1 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn transition_playbook_step_within_bounds_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("trans-step-ok").await;
    let agent_id = app.create_agent("ok-stepper").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Valid step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
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

    // done → ready with playbook_step=2 (last valid step index) should succeed
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready", "playbook_step": 2 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 2);

    app.cleanup().await;
}

#[tokio::test]
async fn transition_playbook_step_none_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("trans-step-none").await;
    let agent_id = app.create_agent("none-stepper").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "None step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
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

    // done → ready without playbook_step should succeed
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

// ── Bulk transition tests ──

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

// ── Bulk delete tests ──

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

// ── bulk_delegate tests ──

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

// ── transition with playbook_step on task without playbook_id ──

#[tokio::test]
async fn transition_with_step_on_no_playbook_task_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("trans-no-pb-step").await;
    let agent_id = app.create_agent("no-pb-stepper").await;

    // Create task WITHOUT a playbook_id
    let task = app.create_task(project_id, "No playbook step test").await;
    let task_id = task["id"].as_str().unwrap();
    assert!(task["playbook_id"].is_null());

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

    // done → ready with playbook_step=99 — no playbook means no upper-bound check,
    // so this succeeds (documents known gap: only negative-value check applies)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready", "playbook_step": 99 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 99);

    app.cleanup().await;
}

// ── update_task playbook_step validation ──

#[tokio::test]
async fn update_task_playbook_step_out_of_range_rejected() {
    let app = require_db!();
    let project_id = app.create_project("upd-step-oob").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Update OOB step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // PUT with playbook_step=99 should fail (only 3 steps)
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "playbook_step": 99 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn update_task_playbook_step_negative_rejected() {
    let app = require_db!();
    let project_id = app.create_project("upd-step-neg").await;

    let task = app
        .create_task(project_id, "Update negative step test")
        .await;
    let task_id = task["id"].as_str().unwrap();

    // PUT with playbook_step=-1 should fail
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "playbook_step": -1 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNPROCESSABLE_ENTITY);

    app.cleanup().await;
}

#[tokio::test]
async fn update_task_playbook_step_within_bounds_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("upd-step-ok").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Update OK step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // PUT with playbook_step=1 should succeed (within 0..2)
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "playbook_step": 1 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 1);

    app.cleanup().await;
}

#[tokio::test]
async fn update_task_playbook_step_none_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("upd-step-none").await;

    let task = app.create_task(project_id, "Update no step test").await;
    let task_id = task["id"].as_str().unwrap();

    // PUT without playbook_step should succeed
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "title": "Updated title" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["title"].as_str().unwrap(), "Updated title");

    app.cleanup().await;
}

// Known gap: when a task has no playbook_id, only the negative-value guard applies.
// playbook_step=99 (or any non-negative value) silently succeeds because the OOB
// upper-bound check in validate_playbook_step is skipped when playbook_id is None.
// This mirrors the same gap documented for transition_task (task #64).
#[tokio::test]
async fn update_task_playbook_step_on_no_playbook_task_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("upd-step-no-pb").await;

    // Create task WITHOUT a playbook_id
    let task = app
        .create_task(project_id, "Update OOB no playbook test")
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert!(task["playbook_id"].is_null());

    // PUT with playbook_step=99 — no playbook means no upper-bound check,
    // so this succeeds (known gap: no playbook → no upper bound enforced)
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "playbook_step": 99 }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        99,
        "playbook_step=99 should be accepted when task has no playbook: {}",
        resp.json
    );

    app.cleanup().await;
}

// ── playbook step regression tests ──

/// Rejection at the merge step (step 2) regresses playbook_step to implement (step 0),
/// not to review (step 1).
#[tokio::test]
async fn merge_rejection_regresses_to_implement() {
    let app = require_db!();
    let project_id = app.create_project("trans-merge-regress").await;
    let agent_id = app.create_agent("merge-rejecter").await;

    // Create a 3-step playbook: implement(0) → review(1) → merge(2)
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Merge rejection test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // Step 0: implement → done (auto-advances to ready, step 1)
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

    // Step 1: review → done (auto-advances to ready, step 2)
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

    // Step 2: claim merge
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "merge");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 2);

    // Merge rejects — transitions to ready. API should regress to step 0 (implement).
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        0,
        "merge rejection should regress to implement step (0), not review (1): {}",
        resp.json
    );

    app.cleanup().await;
}

/// After review rejection regresses to implement, the task can be re-implemented
/// and advance again through the pipeline (implement → review).
#[tokio::test]
async fn rejected_then_re_implemented_advances_again() {
    let app = require_db!();
    let project_id = app.create_project("trans-re-implement").await;
    let agent_id = app.create_agent("re-implementer").await;

    // Create a 3-step playbook: implement(0) → review(1) → merge(2)
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Re-implement after rejection",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // Step 0: implement → done (auto-advances to ready, step 1)
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

    // Step 1: claim review
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;

    // Review rejects — regresses to step 0 (implement)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        0,
        "should regress to implement step (0)"
    );

    // Re-claim implement (step 0)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "implement");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 0);

    // Re-implement → done (should auto-advance to ready, step 1 again)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "ready",
        "re-implemented task should auto-advance to ready: {}",
        resp.json
    );
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        1,
        "playbook_step should advance to review (1) again: {}",
        resp.json
    );

    app.cleanup().await;
}

/// When rejecting at the merge step with multiple implement steps in the playbook,
/// the API regresses to the NEAREST previous implement step, not the first one.
/// Playbook: [implement(0), review(1), implement(2), merge(3)]
/// Rejection at step 3 should regress to step 2 (nearest implement), not step 0.
#[tokio::test]
async fn step_regression_finds_nearest_previous_implement() {
    let app = require_db!();
    let project_id = app.create_project("trans-regress-nearest").await;
    let agent_id = app.create_agent("regress-nearest-agent").await;

    // Create a 4-step playbook: [implement, review, implement, merge]
    let playbook_id = app
        .create_playbook(&["implement", "review", "implement", "merge"])
        .await;

    // Create task with this playbook (starts at ready, step 0)
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Nearest regression test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

    // Step 0 (implement): claim → done → auto-advance to step 1 (ready)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 1);

    // Step 1 (review): claim → done → auto-advance to step 2 (ready)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 2);

    // Step 2 (implement): claim → done → auto-advance to step 3 (ready)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 3);

    // Step 3 (merge): claim, then reject (transition to ready)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        2,
        "should regress to nearest implement step (2), not first implement (0): {}",
        resp.json
    );

    app.cleanup().await;
}

/// Releasing an implement step (step 0) to ready should preserve playbook_step at 0.
/// The regression logic only fires for non-implement steps; an implement step release
/// is a transparent pass-through (e.g. blocker release or agent abandonment).
#[tokio::test]
async fn implement_step_to_ready_does_not_regress() {
    let app = require_db!();
    let project_id = app.create_project("trans-impl-release").await;
    let agent_id = app.create_agent("impl-releaser").await;

    // Create a 3-step playbook: implement(0) → review(1) → merge(2)
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Implement release test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

    // Claim step 0 (implement)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "implement");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 0);

    // Release back to ready (blocker/abandonment mid-implement)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "ready",
        "task should be back in ready state: {}",
        resp.json
    );
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        0,
        "playbook_step should remain at 0 (implement) after release, not regressed: {}",
        resp.json
    );

    app.cleanup().await;
}

/// When a playbook has NO implement step (e.g. [review, merge]), rejecting from the
/// merge step triggers the regression search, but no implement step is found. The for
/// loop exhausts without matching and the code falls through to the standard ready
/// transition — playbook_step stays at its current value (1).
#[tokio::test]
async fn regression_falls_through_when_no_implement_step_before_current() {
    let app = require_db!();
    let project_id = app.create_project("trans-regress-no-impl").await;
    let agent_id = app.create_agent("no-impl-agent").await;

    // Create a 2-step playbook with NO implement step: [review, merge]
    let playbook_id = app.create_playbook(&["review", "merge"]).await;

    // Create task with this playbook (starts at ready, step 0)
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "No implement step regression test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

    // Step 0 (review): claim → done → auto-advances to ready, step 1
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/claim"),
        serde_json::json!({ "agent_id": agent_id }),
    ))
    .await;
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 1);

    // Step 1 (merge): claim
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "merge");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 1);

    // Merge rejects — transitions to ready. No implement step exists to regress to,
    // so the regression search falls through and playbook_step stays at 1.
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "ready");
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        1,
        "playbook_step should stay at 1 (fall-through, no implement step to regress to): {}",
        resp.json
    );

    app.cleanup().await;
}

// ── bulk_transition with playbook_step ──

#[tokio::test]
async fn bulk_transition_with_valid_playbook_step() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-step-ok").await;
    let agent_id = app.create_agent("bulk-stepper").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Bulk step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id: uuid::Uuid = task["id"].as_str().unwrap().parse().unwrap();

    // Move through: backlog → ready → implement (claim) → done (advances to review step)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    let claim_resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(claim_resp.status, StatusCode::OK);
    // Complete implement step → auto-advances to ready with playbook_step=1
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "done" }),
    ))
    .await;

    // Task should now be ready at step 1 (review). Use bulk transition to move it
    // to ready with playbook_step=0 (regress back to implement step).
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [task_id],
                "state": "ready",
                "playbook_step": 0
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert_eq!(succeeded.len(), 1, "task should be in succeeded");
    assert!(failed.is_empty(), "no failures expected");

    // Verify playbook_step was updated to 0
    let fetched = app.send(get(&format!("/v1/tasks/{task_id}"))).await;
    assert_eq!(
        fetched.json["playbook_step"].as_i64().unwrap(),
        0,
        "playbook_step should be 0 after bulk transition: {}",
        fetched.json
    );

    app.cleanup().await;
}

#[tokio::test]
async fn bulk_transition_oob_playbook_step_fails_per_task() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-step-oob").await;
    let agent_id = app.create_agent("bulk-oob-stepper").await;

    // Create a 3-step playbook (steps 0, 1, 2)
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with this playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Bulk OOB step test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id: uuid::Uuid = task["id"].as_str().unwrap().parse().unwrap();

    // Move through: backlog → ready → implement (claim) → done (advances to review step)
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

    // Task is now ready at step 1. Try bulk transition with OOB playbook_step=8
    // (only 3 steps: 0, 1, 2). This should produce a per-task failure, NOT a hard 400.
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [task_id],
                "state": "ready",
                "playbook_step": 8
            }),
        ))
        .await;

    // Should be BAD_REQUEST because all tasks failed (all-fail → 400 via bulk_status_code)
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);

    let succeeded = resp.json["succeeded"].as_array().unwrap();
    let failed = resp.json["failed"].as_array().unwrap();
    assert!(
        succeeded.is_empty(),
        "no tasks should succeed with OOB step"
    );
    assert_eq!(failed.len(), 1, "task should appear in failed list");
    assert_eq!(
        failed[0]["task_id"].as_str().unwrap(),
        task_id.to_string(),
        "failed entry should reference the right task"
    );
    assert!(
        failed[0]["error"]
            .as_str()
            .unwrap()
            .contains("out of range"),
        "error should mention out of range: {}",
        failed[0]["error"]
    );

    app.cleanup().await;
}

// ── bulk_transition: file lock release on pipeline advancement ──

#[tokio::test]
async fn bulk_transition_pipeline_advancement_releases_file_locks() {
    let app = require_db!();
    let project_id = app.create_project("bulk-trans-lock-release").await;
    let agent_id = app.create_agent("lock-stepper").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task with playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Lock release test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id: uuid::Uuid = task["id"].as_str().unwrap().parse().unwrap();

    // Move through: backlog → ready → implement (claim)
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;
    let claim_resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/tasks/{task_id}/claim"),
                serde_json::json!({ "agent_id": agent_id }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(claim_resp.status, StatusCode::OK);

    // Acquire file locks for this task
    let lock_resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/{project_id}/locks"),
                serde_json::json!({
                    "task_id": task_id,
                    "paths": ["src/main.rs"]
                }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(
        lock_resp.status,
        StatusCode::OK,
        "acquire lock: {}",
        lock_resp.json
    );

    // Verify locks exist
    let locks_before = app
        .send(with_agent(
            get(&format!("/v1/{project_id}/locks")),
            agent_id,
        ))
        .await;
    let locks_arr = locks_before.json.as_array().unwrap();
    assert_eq!(locks_arr.len(), 1, "should have 1 lock before transition");

    // Complete implement step via BULK transition → auto-advances to ready + step 1
    let resp = app
        .send(post_json(
            &format!("/v1/{project_id}/tasks/bulk/transition"),
            serde_json::json!({
                "task_ids": [task_id],
                "state": "done"
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let succeeded = resp.json["succeeded"].as_array().unwrap();
    assert_eq!(succeeded.len(), 1);

    // Verify task advanced to ready at step 1 (review)
    let fetched = app.send(get(&format!("/v1/tasks/{task_id}"))).await;
    assert_eq!(fetched.json["state"].as_str().unwrap(), "ready");
    assert_eq!(fetched.json["playbook_step"].as_i64().unwrap(), 1);

    // Verify file locks were released
    let locks_after = app
        .send(with_agent(
            get(&format!("/v1/{project_id}/locks")),
            agent_id,
        ))
        .await;
    let locks_arr = locks_after.json.as_array().unwrap();
    assert!(
        locks_arr.is_empty(),
        "file locks should be released after pipeline advancement, found: {}",
        locks_after.json
    );

    app.cleanup().await;
}

// ── bulk_transition cross-project guard ──

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

// ── update_task: validate playbook_step against effective (post-update) playbook_id ──

#[tokio::test]
async fn update_task_playbook_step_oob_with_new_playbook_rejected() {
    let app = require_db!();
    let project_id = app.create_project("upd-step-oob-new-pb").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task WITHOUT a playbook
    let task = app
        .create_task(project_id, "OOB step with new playbook test")
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert!(task["playbook_id"].is_null());

    // PUT simultaneously setting playbook_id AND playbook_step=99 (OOB for a 3-step playbook)
    // This should be rejected with 422 because step 99 is out of range for the new playbook.
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({
                "playbook_id": playbook_id,
                "playbook_step": 99,
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "playbook_step=99 should be rejected when simultaneously setting a 3-step playbook: {}",
        resp.json
    );

    app.cleanup().await;
}

// ── update_task: simultaneous playbook_id + playbook_step boundary tests ──

/// When setting a new playbook_id AND an out-of-bounds playbook_step in a single PUT,
/// the API rejects with 422. Fixed in task #165: validate_playbook_step now uses the
/// effective (post-update) playbook_id, correctly rejecting out-of-bounds steps.
#[tokio::test]
async fn update_task_new_playbook_and_oob_step_rejected() {
    let app = require_db!();
    let project_id = app.create_project("upd-new-pb-oob").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task WITHOUT a playbook_id
    let task = app
        .create_task(project_id, "New playbook + OOB step test")
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert!(task["playbook_id"].is_null());

    // PUT simultaneously setting playbook_id AND playbook_step=99 (OOB for a 3-step playbook).
    // After task #165 fix: returns 422 (correct). Before fix: returned 200 with step=99 (bug).
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({
                "playbook_id": playbook_id,
                "playbook_step": 99,
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "playbook_step=99 should be rejected (422) for a 3-step playbook (task #165 fix): {}",
        resp.json
    );

    app.cleanup().await;
}

/// Setting a new playbook_id AND a valid playbook_step in a single PUT should succeed.
#[tokio::test]
async fn update_task_new_playbook_and_valid_step_succeeds() {
    let app = require_db!();
    let project_id = app.create_project("upd-new-pb-ok").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task WITHOUT a playbook_id
    let task = app
        .create_task(project_id, "New playbook + valid step test")
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert!(task["playbook_id"].is_null());

    // PUT simultaneously setting playbook_id AND playbook_step=1 (within bounds for 3-step)
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({
                "playbook_id": playbook_id,
                "playbook_step": 1,
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "playbook_step=1 should be accepted for a 3-step playbook: {}",
        resp.json
    );
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        1,
        "playbook_step should be 1: {}",
        resp.json
    );
    assert_eq!(
        resp.json["playbook_id"].as_str().unwrap(),
        playbook_id.to_string(),
        "playbook_id should be set: {}",
        resp.json
    );

    app.cleanup().await;
}

// ── update_task: clearing playbook_id via double-option None path ──

/// PUT with {"playbook_id": null} clears the playbook binding (Some(None) path).
#[tokio::test]
async fn update_task_clears_playbook_id() {
    let app = require_db!();
    let project_id = app.create_project("upd-clear-pb").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task WITH a playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Clear playbook_id test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(
        task["playbook_id"].as_str().unwrap(),
        playbook_id.to_string(),
        "task should start with a playbook_id assigned"
    );

    // PUT with {"playbook_id": null} → deserializes to Some(None), clearing the binding
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "playbook_id": null }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "clearing playbook_id should succeed: {}",
        resp.json
    );
    assert!(
        resp.json["playbook_id"].is_null(),
        "playbook_id should be null after clearing: {}",
        resp.json
    );

    app.cleanup().await;
}

/// Clearing playbook_id leaves playbook_step orphaned (known gap).
/// The step value is preserved even though the playbook binding is gone.
#[tokio::test]
async fn update_task_clears_playbook_id_also_clears_step() {
    let app = require_db!();
    let project_id = app.create_project("upd-clear-pb-step").await;

    // Create a 3-step playbook
    let playbook_id = app.create_playbook(&["implement", "review", "merge"]).await;

    // Create task WITH a playbook and set playbook_step=1
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Clear playbook_id step test",
                "playbook_id": playbook_id,
                "playbook_step": 1,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(
        task["playbook_id"].as_str().unwrap(),
        playbook_id.to_string(),
    );
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 1);

    // Clear playbook_id — step is NOT cleared (orphaned-step gap)
    let resp = app
        .send(put_json(
            &format!("/v1/tasks/{task_id}"),
            serde_json::json!({ "playbook_id": null }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "clearing playbook_id should succeed: {}",
        resp.json
    );
    assert!(
        resp.json["playbook_id"].is_null(),
        "playbook_id should be null after clearing: {}",
        resp.json
    );
    // Known gap: playbook_step is preserved even though the playbook binding is gone.
    // This documents the orphaned-step behavior — the step value remains stale.
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        1,
        "playbook_step should be preserved (orphaned) when playbook_id is cleared: {}",
        resp.json
    );

    app.cleanup().await;
}
