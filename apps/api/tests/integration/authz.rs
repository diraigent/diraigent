use crate::harness::*;
use axum::http::StatusCode;
use uuid::Uuid;

/// Human user (no X-Agent-Id header) bypasses all authz checks.
#[tokio::test]
async fn human_user_can_access_without_membership() {
    let app = require_db!();
    let project_id = app.create_project("authz-human").await;

    // No X-Agent-Id → human user, should work
    let task = app.create_task(project_id, "Human task").await;
    assert_eq!(task["title"].as_str().unwrap(), "Human task");

    app.cleanup().await;
}

/// Agent without membership cannot list tasks.
#[tokio::test]
async fn agent_without_membership_is_rejected() {
    let app = require_db!();
    let project_id = app.create_project("authz-nomember").await;
    let agent_id = app.create_agent("outsider").await;

    let resp = app
        .send(with_agent(
            get(&format!("/v1/{project_id}/tasks")),
            agent_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

/// Agent with execute authority can claim tasks.
#[tokio::test]
async fn agent_with_execute_can_claim() {
    let app = require_db!();
    let project_id = app.create_project("authz-exec").await;
    let agent_id = app.create_agent("executor").await;
    let role_id = app.create_role(project_id, "Worker", &["execute"]).await;
    app.add_member(project_id, agent_id, role_id).await;

    // Create task as human, transition to ready
    let task = app.create_task(project_id, "Claimable").await;
    let task_id = task["id"].as_str().unwrap();
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;

    // Agent claims the task
    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/tasks/{task_id}/claim"),
                serde_json::json!({ "agent_id": agent_id }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "claim failed: {}", resp.json);
    assert_eq!(resp.json["state"].as_str().unwrap(), "working");

    app.cleanup().await;
}

/// Agent without execute authority cannot claim tasks.
#[tokio::test]
async fn agent_without_execute_cannot_claim() {
    let app = require_db!();
    let project_id = app.create_project("authz-noexec").await;
    let agent_id = app.create_agent("reviewer-only").await;
    let role_id = app.create_role(project_id, "Reviewer", &["review"]).await;
    app.add_member(project_id, agent_id, role_id).await;

    // Create & ready a task
    let task = app.create_task(project_id, "Protected").await;
    let task_id = task["id"].as_str().unwrap();
    app.send(post_json(
        &format!("/v1/tasks/{task_id}/transition"),
        serde_json::json!({ "state": "ready" }),
    ))
    .await;

    // Agent with only review cannot claim a non-review step
    // For a task without playbook, step is "working" which requires "execute"
    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/tasks/{task_id}/claim"),
                serde_json::json!({ "agent_id": agent_id }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

/// Agent with create authority can create tasks.
#[tokio::test]
async fn agent_with_create_can_create_tasks() {
    let app = require_db!();
    let project_id = app.create_project("authz-create").await;
    let agent_id = app.create_agent("creator").await;
    let role_id = app.create_role(project_id, "Creator", &["create"]).await;
    app.add_member(project_id, agent_id, role_id).await;

    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/{project_id}/tasks"),
                serde_json::json!({ "title": "Agent-created task" }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create failed: {}", resp.json);
    assert_eq!(resp.json["title"].as_str().unwrap(), "Agent-created task");

    app.cleanup().await;
}

/// Agent without create authority cannot create tasks.
#[tokio::test]
async fn agent_without_create_cannot_create_tasks() {
    let app = require_db!();
    let project_id = app.create_project("authz-nocreate").await;
    let agent_id = app.create_agent("worker-only").await;
    let role_id = app.create_role(project_id, "Worker", &["execute"]).await;
    app.add_member(project_id, agent_id, role_id).await;

    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/{project_id}/tasks"),
                serde_json::json!({ "title": "Unauthorized" }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::UNAUTHORIZED);

    app.cleanup().await;
}

/// Agent with delegate authority can delegate tasks.
#[tokio::test]
async fn agent_with_delegate_can_delegate() {
    let app = require_db!();
    let project_id = app.create_project("authz-delegate").await;
    let delegator_id = app.create_agent("delegator").await;
    let worker_id = app.create_agent("target-worker").await;

    let manager_role = app
        .create_role(project_id, "Manager", &["delegate", "execute"])
        .await;
    let worker_role = app.create_role(project_id, "Worker", &["execute"]).await;
    app.add_member(project_id, delegator_id, manager_role).await;
    app.add_member(project_id, worker_id, worker_role).await;

    let task = app.create_task(project_id, "Delegatable").await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/tasks/{task_id}/delegate"),
                serde_json::json!({ "agent_id": worker_id }),
            ),
            delegator_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "delegate: {}", resp.json);
    assert_eq!(
        resp.json["assigned_agent_id"].as_str().unwrap(),
        worker_id.to_string()
    );

    app.cleanup().await;
}

/// Authority inheritance: agent with manage on parent project has authority on child.
#[tokio::test]
async fn manage_authority_inherits_to_child_project() {
    let app = require_db!();
    let parent_id = app.create_project("authz-parent").await;
    let agent_id = app.create_agent("manager").await;

    // Create role with manage authority on parent
    let role_id = app.create_role(parent_id, "Superadmin", &["manage"]).await;
    app.add_member(parent_id, agent_id, role_id).await;

    // Create child project
    let resp = app
        .send(post_json(
            "/v1",
            serde_json::json!({
                "name": "authz-child",
                "parent_id": parent_id,
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let child_id = resp.id();

    // Agent should be able to create tasks in child (inherited manage authority)
    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/{child_id}/tasks"),
                serde_json::json!({ "title": "Inherited access" }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "inherited create failed: {}",
        resp.json
    );

    app.cleanup().await;
}

/// Spoofing a non-existent agent ID in X-Agent-Id is rejected (agent not in DB).
#[tokio::test]
async fn spoofed_nonexistent_agent_id_is_rejected() {
    let app = require_db!();
    let project_id = app.create_project("authz-spoof-nonexistent").await;

    let fake_id = Uuid::now_v7();
    let resp = app
        .send(with_agent(get(&format!("/v1/{project_id}/tasks")), fake_id))
        .await;
    // A non-existent agent can't be validated → rejected
    assert_eq!(resp.status, StatusCode::FORBIDDEN);

    app.cleanup().await;
}

/// An agent registered by the dev user can be used by that same user.
#[tokio::test]
async fn owner_can_use_their_own_agent_id() {
    let app = require_db!();
    let project_id = app.create_project("authz-own-agent").await;
    let agent_id = app.create_agent("owned-agent").await;
    let role_id = app.create_role(project_id, "Worker", &["execute"]).await;
    app.add_member(project_id, agent_id, role_id).await;

    // The agent was registered by the dev user (DEV_USER_ID). Using the same
    // auth context, the agent_id should be accepted.
    let resp = app
        .send(with_agent(
            get(&format!("/v1/{project_id}/tasks")),
            agent_id,
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "owner should be allowed: {}",
        resp.json
    );

    app.cleanup().await;
}
