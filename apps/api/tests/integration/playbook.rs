use crate::harness::*;
use axum::http::StatusCode;
use uuid::Uuid;

/// Helper: create a global playbook and return it.
async fn create_default_playbook(app: &TestApp) -> serde_json::Value {
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Default Pipeline",
                "trigger_description": "implement → review → merge",
                "steps": [
                    {"name": "implement"},
                    {"name": "review"},
                    {"name": "merge"}
                ],
                "metadata": {"default": true},
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create playbook: {}",
        resp.json
    );
    resp.json
}

/// Global playbooks can be listed without a project scope.
#[tokio::test]
async fn global_playbooks_list() {
    let app = require_db!();
    let playbook = create_default_playbook(&app).await;

    let resp = app.send(get("/v1/playbooks")).await;
    assert_eq!(resp.status, StatusCode::OK);
    let playbooks = resp.json.as_array().unwrap();
    assert!(
        playbooks.iter().any(|p| p["id"] == playbook["id"]),
        "listed playbooks should include the created one"
    );

    app.cleanup().await;
}

/// Task created with playbook_id auto-transitions to ready state.
#[tokio::test]
async fn task_with_playbook_starts_ready() {
    let app = require_db!();
    let project_id = app.create_project("pb-autoready").await;
    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Pipeline task",
                "playbook_id": playbook_id,
            }),
        )
        .await;

    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);
    assert_eq!(task["playbook_id"].as_str().unwrap(), playbook_id);

    app.cleanup().await;
}

/// Claiming a task with playbook enters the first step name ("implement").
#[tokio::test]
async fn claim_enters_first_playbook_step() {
    let app = require_db!();
    let project_id = app.create_project("pb-claim").await;
    let agent_id = app.create_agent("implementer").await;
    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Implement me",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "claim: {}", resp.json);
    assert_eq!(resp.json["state"].as_str().unwrap(), "implement");

    app.cleanup().await;
}

/// Full pipeline: implement → (auto-advance) → review → (auto-advance) → merge → done.
/// The API now handles pipeline advancement atomically — transitioning a non-final
/// step to "done" automatically sets state="ready" with the next playbook_step.
/// "done" is only reached on the final step.
#[tokio::test]
async fn full_playbook_pipeline() {
    let app = require_db!();
    let project_id = app.create_project("pb-full").await;
    let agent_id = app.create_agent("pipeline-agent").await;
    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Full pipeline",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // Step 0: implement
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "implement");

    // implement → "done" request → API auto-advances to ready (step 1)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "ready",
        "non-final step should auto-advance to ready: {}",
        resp.json
    );
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        1,
        "playbook_step should be bumped to 1"
    );
    assert!(
        resp.json["completed_at"].is_null(),
        "completed_at should not be set on advancement"
    );

    // Step 1: review
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "review",
        "step 1 should be review: {}",
        resp.json
    );

    // review → "done" request → API auto-advances to ready (step 2)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "ready",
        "non-final step should auto-advance to ready: {}",
        resp.json
    );
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        2,
        "playbook_step should be bumped to 2"
    );

    // Step 2: merge
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "merge",
        "step 2 should be merge: {}",
        resp.json
    );

    // merge → done (final step — actually enters "done")
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "done");
    assert!(resp.json["completed_at"].as_str().is_some());

    app.cleanup().await;
}

/// Focused test: a single transition from a non-final step to "done" atomically
/// auto-advances the task to state="ready" with playbook_step incremented.
/// This documents the invariant introduced by task #50 (API auto-advancement).
#[tokio::test]
async fn step_done_auto_advances_to_ready() {
    let app = require_db!();
    let project_id = app.create_project("pb-auto-adv").await;
    let agent_id = app.create_agent("auto-adv-agent").await;
    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Auto advance test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_step"].as_i64().unwrap(), 0);

    // Claim → enters step 0 ("implement")
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["state"].as_str().unwrap(), "implement");

    // Single transition to "done" — API should intercept and return ready+step 1
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "transition: {}", resp.json);
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "ready",
        "non-final step done must auto-advance to ready"
    );
    assert_eq!(
        resp.json["playbook_step"].as_i64().unwrap(),
        1,
        "playbook_step must be incremented to 1"
    );
    assert!(
        resp.json["completed_at"].is_null(),
        "completed_at must not be set on auto-advancement"
    );

    app.cleanup().await;
}

/// Focused test: transitioning the final playbook step to "done" stays in done
/// state with completed_at set. No auto-advancement occurs.
#[tokio::test]
async fn final_step_done_stays_done() {
    let app = require_db!();
    let project_id = app.create_project("pb-final-done").await;
    let agent_id = app.create_agent("final-done-agent").await;
    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Final step done test",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // Walk through all steps to reach the final one (step 2 = merge)
    // Step 0: implement
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

    // Step 1: review
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

    // Step 2: merge (final step)
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "merge");
    assert_eq!(resp.json["playbook_step"].as_i64().unwrap(), 2);

    // Final step → done: must stay in done, not auto-advance
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "done" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "transition: {}", resp.json);
    assert_eq!(
        resp.json["state"].as_str().unwrap(),
        "done",
        "final step must stay in done state"
    );
    assert!(
        resp.json["completed_at"].as_str().is_some(),
        "completed_at must be set when truly done"
    );

    app.cleanup().await;
}

/// Review rejection regresses playbook_step to the previous implement step.
/// The API handles this atomically when a non-implement step transitions to ready.
#[tokio::test]
async fn review_rejection_regresses_to_implement() {
    let app = require_db!();
    let project_id = app.create_project("pb-regress").await;
    let agent_id = app.create_agent("regress-agent").await;
    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Rejection pipeline",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // Step 0: implement → advance to review
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

    // Review rejects — transitions to ready. API should regress to step 0.
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
        "should regress to implement step (0): {}",
        resp.json
    );

    app.cleanup().await;
}

/// Custom playbook with different step names.
#[tokio::test]
async fn custom_playbook_steps() {
    let app = require_db!();
    let project_id = app.create_project("pb-custom").await;
    let agent_id = app.create_agent("custom-agent").await;

    // Create a custom global playbook
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Two-Step",
                "steps": [
                    { "name": "develop" },
                    { "name": "verify" },
                ],
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create playbook: {}",
        resp.json
    );
    let playbook_id = resp.json["id"].as_str().unwrap();

    // Create task with custom playbook
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Custom pipeline",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["state"].as_str().unwrap(), "ready");

    // Claim → enters "develop" step
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/claim"),
            serde_json::json!({ "agent_id": agent_id }),
        ))
        .await;
    assert_eq!(resp.json["state"].as_str().unwrap(), "develop");

    app.cleanup().await;
}

/// Review step accepts both execute and review authorities.
#[tokio::test]
async fn review_step_accepts_review_authority() {
    let app = require_db!();
    let project_id = app.create_project("pb-review-auth").await;
    let agent_id = app.create_agent("reviewer").await;

    // Agent with only review authority
    let role_id = app.create_role(project_id, "Reviewer", &["review"]).await;
    app.add_member(project_id, agent_id, role_id).await;

    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    // Create task, advance to review step
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Review me",
                "playbook_id": playbook_id,
            }),
        )
        .await;
    let task_id = task["id"].as_str().unwrap();

    // Advance playbook_step to 1 (review)
    app.send(put_json(
        &format!("/v1/tasks/{task_id}"),
        serde_json::json!({ "playbook_step": 1 }),
    ))
    .await;

    // Agent with review authority can claim the review step
    let resp = app
        .send(with_agent(
            post_json(
                &format!("/v1/tasks/{task_id}/claim"),
                serde_json::json!({ "agent_id": agent_id }),
            ),
            agent_id,
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "review claim: {}", resp.json);
    assert_eq!(resp.json["state"].as_str().unwrap(), "review");

    app.cleanup().await;
}

/// Project default_playbook_id is used when creating tasks without explicit playbook.
#[tokio::test]
async fn project_default_playbook() {
    let app = require_db!();
    let project_id = app.create_project("pb-default-proj").await;

    let playbook = create_default_playbook(&app).await;
    let playbook_id = playbook["id"].as_str().unwrap();

    // Set project's default playbook
    let resp = app
        .send(put_json(
            &format!("/v1/{project_id}"),
            serde_json::json!({ "default_playbook_id": playbook_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "update project: {}", resp.json);
    assert_eq!(
        resp.json["default_playbook_id"].as_str().unwrap(),
        playbook_id
    );

    // Task created without playbook_id should get the default
    let task = app
        .create_task_with(
            project_id,
            serde_json::json!({ "title": "Auto-playbook task" }),
        )
        .await;
    assert_eq!(task["state"].as_str().unwrap(), "ready");
    assert_eq!(task["playbook_id"].as_str().unwrap(), playbook_id);

    app.cleanup().await;
}

/// Helper: insert a default (tenant_id = NULL) playbook directly into the DB.
async fn insert_default_playbook(app: &TestApp) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO diraigent.playbook (id, tenant_id, title, trigger_description, steps, tags, metadata, initial_state, created_by)
         VALUES ($1, NULL, 'System Default', NULL, '[]'::jsonb, '{}', '{}'::jsonb, 'ready', '00000000-0000-0000-0000-000000000000')",
    )
    .bind(id)
    .execute(&app.pool)
    .await
    .expect("insert default playbook");
    id
}

/// Updating a default playbook (tenant_id IS NULL) must return 200 and fork it into a
/// tenant-owned copy (copy-on-write semantics), leaving the original untouched.
#[tokio::test]
async fn default_playbook_update_forks_copy() {
    let app = require_db!();
    let default_id = insert_default_playbook(&app).await;

    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{default_id}"),
            serde_json::json!({ "title": "My Custom Version" }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "expected 200 forking default playbook: {}",
        resp.json
    );

    // The returned playbook must be a NEW record (different id).
    let forked_id = resp.json["id"].as_str().unwrap();
    assert_ne!(forked_id, default_id.to_string(), "fork must have a new id");

    // The fork must be owned by the tenant (tenant_id not null).
    assert!(
        !resp.json["tenant_id"].is_null(),
        "fork must have a tenant_id"
    );

    // The requested title change must be applied on the fork.
    assert_eq!(resp.json["title"].as_str().unwrap(), "My Custom Version");

    // The original default playbook must remain unchanged.
    let original = app.send(get(&format!("/v1/playbooks/{default_id}"))).await;
    assert_eq!(original.status, StatusCode::OK);
    assert_eq!(original.json["title"].as_str().unwrap(), "System Default");
    assert!(
        original.json["tenant_id"].is_null(),
        "original must stay null"
    );

    app.cleanup().await;
}

/// Deleting a default playbook (tenant_id IS NULL) must return 403 Forbidden.
#[tokio::test]
async fn default_playbook_delete_is_forbidden() {
    let app = require_db!();
    let id = insert_default_playbook(&app).await;

    let resp = app.send(delete(&format!("/v1/playbooks/{id}"))).await;
    assert_eq!(
        resp.status,
        StatusCode::FORBIDDEN,
        "expected 403 deleting default playbook: {}",
        resp.json
    );

    app.cleanup().await;
}

/// Forking a default playbook records parent_id and parent_version on the fork.
#[tokio::test]
async fn fork_records_parent_lineage() {
    let app = require_db!();
    let default_id = insert_default_playbook(&app).await;

    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{default_id}"),
            serde_json::json!({ "title": "Forked" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "fork: {}", resp.json);

    let forked = &resp.json;
    assert_ne!(forked["id"].as_str().unwrap(), default_id.to_string());
    assert_eq!(
        forked["parent_id"].as_str().unwrap(),
        default_id.to_string(),
        "fork should reference parent"
    );
    assert_eq!(
        forked["parent_version"].as_i64().unwrap(),
        1,
        "fork should record parent version at time of fork"
    );
    assert_eq!(
        forked["version"].as_i64().unwrap(),
        1,
        "fork starts at version 1"
    );

    app.cleanup().await;
}

/// Updating a tenant-owned playbook increments its version.
#[tokio::test]
async fn update_increments_version() {
    let app = require_db!();
    let playbook = create_default_playbook(&app).await;
    let id = playbook["id"].as_str().unwrap();
    assert_eq!(playbook["version"].as_i64().unwrap(), 1);

    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{id}"),
            serde_json::json!({ "title": "v2 title" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["version"].as_i64().unwrap(), 2);

    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{id}"),
            serde_json::json!({ "title": "v3 title" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(resp.json["version"].as_i64().unwrap(), 3);

    app.cleanup().await;
}

/// Syncing a forked playbook updates its content from the parent.
#[tokio::test]
async fn sync_updates_fork_from_parent() {
    let app = require_db!();
    let default_id = insert_default_playbook(&app).await;

    // Fork the default playbook
    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{default_id}"),
            serde_json::json!({ "title": "My Fork" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let fork_id = resp.json["id"].as_str().unwrap().to_string();
    assert_eq!(resp.json["parent_version"].as_i64().unwrap(), 1);

    // Update the default playbook directly in the DB (simulating a version bump).
    sqlx::query(
        "UPDATE diraigent.playbook SET title = 'System Default v2', version = 2,
         steps = '[{\"name\": \"implement\"}, {\"name\": \"review\"}]'::jsonb
         WHERE id = $1",
    )
    .bind(default_id)
    .execute(&app.pool)
    .await
    .expect("update default playbook");

    // Sync the fork
    let resp = app
        .send(post_json(
            &format!("/v1/playbooks/{fork_id}/sync"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "sync: {}", resp.json);

    // Fork should now have parent's steps and updated parent_version
    assert_eq!(
        resp.json["parent_version"].as_i64().unwrap(),
        2,
        "parent_version should be updated after sync"
    );
    let steps = resp.json["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2, "steps should be synced from parent");
    // Title stays as-is (sync doesn't override title)
    assert_eq!(resp.json["title"].as_str().unwrap(), "My Fork");
    // Version should be incremented
    assert_eq!(
        resp.json["version"].as_i64().unwrap(),
        2,
        "fork version should increment after sync"
    );

    app.cleanup().await;
}

/// Syncing a playbook without a parent returns an error.
#[tokio::test]
async fn sync_without_parent_fails() {
    let app = require_db!();
    let playbook = create_default_playbook(&app).await;
    let id = playbook["id"].as_str().unwrap();

    let resp = app
        .send(post_json(
            &format!("/v1/playbooks/{id}/sync"),
            serde_json::json!({}),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::BAD_REQUEST,
        "sync without parent: {}",
        resp.json
    );

    app.cleanup().await;
}

/// Tenant-owned playbooks can still be updated and deleted normally.
#[tokio::test]
async fn tenant_playbook_is_mutable() {
    let app = require_db!();
    let playbook = create_default_playbook(&app).await;
    let id = playbook["id"].as_str().unwrap();

    // Update should succeed
    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{id}"),
            serde_json::json!({ "title": "Updated Title" }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "expected 200 updating tenant playbook: {}",
        resp.json
    );
    assert_eq!(resp.json["title"].as_str().unwrap(), "Updated Title");

    // Delete should succeed
    let resp = app.send(delete(&format!("/v1/playbooks/{id}"))).await;
    assert_eq!(
        resp.status,
        StatusCode::NO_CONTENT,
        "expected 204 deleting tenant playbook: {}",
        resp.json
    );

    app.cleanup().await;
}

// ── Step template reference tests ──

/// Helper: insert a step template directly and return its UUID.
async fn insert_step_template(app: &TestApp, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO diraigent.step_template
         (id, tenant_id, name, description, model, budget, allowed_tools, tags, metadata, created_by)
         VALUES ($1, NULL, $2, 'Test template description', 'opus', 12.0, 'full', '{}', '{}'::jsonb,
                 '00000000-0000-0000-0000-000000000000')",
    )
    .bind(id)
    .bind(name)
    .execute(&app.pool)
    .await
    .expect("insert step template");
    id
}

/// Creating a playbook with valid step_template_id references succeeds.
#[tokio::test]
async fn playbook_with_valid_step_template_ids() {
    let app = require_db!();
    let tmpl_id = insert_step_template(&app, "implement-tmpl").await;

    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Template-based Pipeline",
                "steps": [
                    {"name": "implement", "step_template_id": tmpl_id.to_string()},
                    {"name": "review"}
                ],
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::OK,
        "create playbook with template ref: {}",
        resp.json
    );

    // Verify steps are stored with step_template_id
    let steps = resp.json["steps"].as_array().unwrap();
    assert_eq!(
        steps[0]["step_template_id"].as_str().unwrap(),
        tmpl_id.to_string()
    );
    // Second step has no template ref
    assert!(steps[1].get("step_template_id").is_none());

    app.cleanup().await;
}

/// Creating a playbook with invalid step_template_id returns 400.
#[tokio::test]
async fn playbook_with_invalid_step_template_id_rejected() {
    let app = require_db!();
    let bogus_id = Uuid::new_v4();

    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Bad Template Ref",
                "steps": [
                    {"name": "implement", "step_template_id": bogus_id.to_string()},
                ],
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::BAD_REQUEST,
        "expected 400 for invalid template ref: {}",
        resp.json
    );

    app.cleanup().await;
}

/// GET playbook returns resolved_steps when steps reference templates.
#[tokio::test]
async fn get_playbook_returns_resolved_steps() {
    let app = require_db!();
    let tmpl_id = insert_step_template(&app, "implement-resolved").await;

    // Create playbook with a step referencing the template
    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Resolved Pipeline",
                "steps": [
                    {"name": "my-impl", "step_template_id": tmpl_id.to_string(), "budget": 20.0},
                    {"name": "review"}
                ],
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "create: {}", resp.json);
    let playbook_id = resp.json["id"].as_str().unwrap();

    // GET should include resolved_steps
    let resp = app.send(get(&format!("/v1/playbooks/{playbook_id}"))).await;
    assert_eq!(resp.status, StatusCode::OK, "get: {}", resp.json);

    // resolved_steps should be present
    let resolved = resp.json["resolved_steps"].as_array();
    assert!(
        resolved.is_some(),
        "resolved_steps should be present when templates are used: {}",
        resp.json
    );
    let resolved = resolved.unwrap();
    assert_eq!(resolved.len(), 2);

    // First step should have template values merged with inline overrides
    let first = &resolved[0];
    // Inline "name" = "my-impl" overrides template "name" = "implement-resolved"
    assert_eq!(first["name"].as_str().unwrap(), "my-impl");
    // Inline budget=20 overrides template budget=12
    assert_eq!(first["budget"].as_f64().unwrap(), 20.0);
    // Template model=opus should be inherited
    assert_eq!(first["model"].as_str().unwrap(), "opus");
    // Template allowed_tools=full should be inherited
    assert_eq!(first["allowed_tools"].as_str().unwrap(), "full");
    // Template description should be inherited
    assert!(first["description"].as_str().is_some());
    // step_template_id should still be present for audit
    assert_eq!(
        first["step_template_id"].as_str().unwrap(),
        tmpl_id.to_string()
    );

    // Second step (no template) should be unchanged
    let second = &resolved[1];
    assert_eq!(second["name"].as_str().unwrap(), "review");
    assert!(second.get("step_template_id").is_none());

    app.cleanup().await;
}

/// GET playbook without template references omits resolved_steps.
#[tokio::test]
async fn get_playbook_without_templates_no_resolved_steps() {
    let app = require_db!();

    let resp = app
        .send(post_json(
            "/v1/playbooks",
            serde_json::json!({
                "title": "Plain Pipeline",
                "steps": [
                    {"name": "implement"},
                    {"name": "review"}
                ],
            }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);
    let playbook_id = resp.json["id"].as_str().unwrap();

    let resp = app.send(get(&format!("/v1/playbooks/{playbook_id}"))).await;
    assert_eq!(resp.status, StatusCode::OK, "get: {}", resp.json);

    // resolved_steps should NOT be present (no templates used)
    assert!(
        resp.json.get("resolved_steps").is_none() || resp.json["resolved_steps"].is_null(),
        "resolved_steps should be absent when no templates used: {}",
        resp.json
    );

    app.cleanup().await;
}

/// Updating a playbook with invalid step_template_id is rejected.
#[tokio::test]
async fn update_playbook_invalid_step_template_id_rejected() {
    let app = require_db!();
    let playbook = create_default_playbook(&app).await;
    let id = playbook["id"].as_str().unwrap();
    let bogus_id = Uuid::new_v4();

    let resp = app
        .send(put_json(
            &format!("/v1/playbooks/{id}"),
            serde_json::json!({
                "steps": [
                    {"name": "implement", "step_template_id": bogus_id.to_string()},
                ],
            }),
        ))
        .await;
    assert_eq!(
        resp.status,
        StatusCode::BAD_REQUEST,
        "expected 400 for invalid template ref on update: {}",
        resp.json
    );

    app.cleanup().await;
}
