#[allow(dead_code, unused_macros)]
#[macro_use]
#[path = "integration/harness.rs"]
mod harness;
use axum::http::StatusCode;
use diraigent_api::ws_protocol::WsMessage;
use diraigent_types::project_content::*;
use harness::*;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

type Payloads = Arc<Mutex<HashMap<(Uuid, String, Uuid), Value>>>;
fn connect(app: &TestApp, agent: Uuid, store_id: Uuid) -> Payloads {
    let data: Payloads = Arc::default();
    let values = data.clone();
    let registry = app.ws_registry.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    assert!(registry.try_register(agent, tx));
    tokio::spawn(async move {
        while let Some(WsMessage::ContentRequest {
            request_id,
            project_id,
            request,
            expected_store_id,
        }) = rx.recv().await
        {
            let result = if expected_store_id.is_some_and(|id| id != store_id) {
                Err(ContentError::Unavailable)
            } else {
                match request {
                    ContentRequest::Probe => Ok(json!({"protocol":1,"store_id":store_id})),
                    ContentRequest::Put { kind, id, value } => {
                        let mut values = values.lock().unwrap();
                        let key = (project_id, kind.as_str().into(), id);
                        if values.get(&key).is_some_and(|old| old != &value) {
                            Err(ContentError::Conflict)
                        } else {
                            values.insert(key, value);
                            Ok(json!({"stored":true}))
                        }
                    }
                    ContentRequest::Get { kind, id } => values
                        .lock()
                        .unwrap()
                        .get(&(project_id, kind.as_str().into(), id))
                        .cloned()
                        .ok_or(ContentError::NotFound),
                    ContentRequest::History { user_id } => Ok(
                        json!({"enabled":true,"revision":0,"messages":[],"busy":false,"authenticated_user":user_id}),
                    ),
                    ContentRequest::ClearHistory { .. } => {
                        Ok(json!({"enabled":true,"revision":1,"messages":[],"busy":false}))
                    }
                }
            };
            registry.complete_content_request(agent, &request_id, result);
        }
    });
    data
}

#[tokio::test]
async fn migrate_and_create_payloads_without_central_retention_and_fail_closed_offline() {
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Stored project").await;
    let task = app.create_task(project, "Work").await;
    let task = task["id"].as_str().unwrap();
    let log = app
        .send(post_json(
            &format!("/v1/{project}/task-logs"),
            json!({"task_id":task,"content":"private-log","metadata":{"private":"log-meta"}}),
        ))
        .await;
    assert_eq!(log.status, StatusCode::OK);
    let log_id = log.json["id"].as_str().unwrap();
    let diff = app
        .send(post_json(
            &format!("/v1/tasks/{task}/changed-files"),
            json!({"files":[{"path":"src/a.rs","change_type":"modified","diff":"private-diff"}]}),
        ))
        .await;
    assert_eq!(diff.status, StatusCode::OK);
    let diff_id = diff.json[0]["id"].as_str().unwrap();
    let artifact=app.send(post_json(&format!("/v1/tasks/{task}/updates"),json!({"kind":"artifact","content":"private-artifact","metadata":{"private":"artifact-meta"}}))).await;
    assert_eq!(artifact.status, StatusCode::OK);
    let agent = app.create_agent("Storage owner").await;
    let store_id = Uuid::new_v4();
    let values = connect(&app, agent, store_id);
    let assigned = app
        .send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":agent}),
        ))
        .await;
    assert_eq!(assigned.status, StatusCode::OK, "{}", assigned.json);
    let migrated = app
        .send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({}),
        ))
        .await;
    assert_eq!(migrated.status, StatusCode::OK, "{}", migrated.json);
    assert_eq!(migrated.json["moved"], 3);
    assert_eq!(
        app.send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({})
        ))
        .await
        .json["moved"],
        0
    );
    assert_eq!(values.lock().unwrap().len(), 3);
    assert_eq!(
        app.send(get(&format!("/v1/task-logs/{log_id}"))).await.json["content"],
        "private-log"
    );
    assert_eq!(
        app.send(get(&format!("/v1/tasks/{task}/changed-files/{diff_id}")))
            .await
            .json["diff"],
        "private-diff"
    );
    let updates = app.send(get(&format!("/v1/tasks/{task}/updates"))).await;
    assert_eq!(updates.json[0]["content"], "private-artifact");
    let central: String =
        sqlx::query_scalar("SELECT content FROM diraigent.task_log WHERE id=$1::uuid")
            .bind(log_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(central.is_empty());
    let central: String =
        sqlx::query_scalar("SELECT content FROM diraigent.task_update WHERE id=$1::uuid")
            .bind(artifact.json["id"].as_str().unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(central.is_empty());
    let new = app
        .send(post_json(
            &format!("/v1/{project}/task-logs"),
            json!({"task_id":task,"content":"new-private-log"}),
        ))
        .await;
    assert_eq!(new.status, StatusCode::OK);
    let history = app
        .send(get(&format!(
            "/v1/{project}/chat/history?user_id={}",
            Uuid::new_v4()
        )))
        .await;
    let account = app.send(get("/v1/account")).await;
    assert_eq!(history.json["authenticated_user"], account.json["user_id"]);
    assert_eq!(
        app.send(post_json(
            &format!("/v1/{project}/chat/history/clear"),
            json!({"revision":0})
        ))
        .await
        .status,
        StatusCode::OK
    );
    let tenant: Uuid = sqlx::query_scalar("SELECT tenant_id FROM diraigent.project WHERE id=$1")
        .bind(project)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        app.send(post_json(
            &format!("/v1/tenants/{tenant}/encryption/init"),
            json!({})
        ))
        .await
        .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        app.send(put_json(
            &format!("/v1/tenants/{tenant}"),
            json!({"encryption_mode":"login_derived"})
        ))
        .await
        .status,
        StatusCode::CONFLICT
    );
    let error =
        sqlx::query("UPDATE diraigent.tenant SET encryption_mode='login_derived' WHERE id=$1")
            .bind(tenant)
            .execute(&app.pool)
            .await
            .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    sqlx::query("UPDATE diraigent.agent SET status='revoked' WHERE id=$1")
        .bind(agent)
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        app.send(get(&format!("/v1/{project}/chat/history")))
            .await
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    sqlx::query("UPDATE diraigent.agent SET status='idle' WHERE id=$1")
        .bind(agent)
        .execute(&app.pool)
        .await
        .unwrap();
    app.ws_registry.unregister(agent);
    // Another connected worker must not serve the owner's content.
    let other = app.create_agent("Other worker").await;
    connect(&app, other, Uuid::new_v4());
    assert_eq!(
        app.send(get(&format!("/v1/task-logs/{log_id}")))
            .await
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        app.send(get(&format!("/v1/{project}/task-logs")))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.send(post_json(
            &format!("/v1/{project}/task-logs"),
            json!({"task_id":task,"content":"must-not-fall-back"})
        ))
        .await
        .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        app.send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":other})
        ))
        .await
        .status,
        StatusCode::CONFLICT
    );
    app.cleanup().await;
}

#[tokio::test]
async fn storage_assignment_requires_manage_and_rejects_cross_workspace_and_changed_store() {
    let app = TestApp::try_new().await.expect("requires PostgreSQL");
    let project = app.create_project("Owned").await;
    let agent = app.create_agent("Owner").await;
    let store = Uuid::new_v4();
    connect(&app, agent, store);
    assert_eq!(
        app.send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":agent})
        ))
        .await
        .status,
        StatusCode::OK
    );
    app.ws_registry.unregister(agent);
    connect(&app, agent, Uuid::new_v4());
    assert_eq!(
        app.send(get(&format!("/v1/{project}/chat/history")))
            .await
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let other = app.create_agent("Outside").await;
    app.remove_agent_memberships(other).await;
    assert_eq!(
        app.send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":other})
        ))
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    let account = app.send(get("/v1/account")).await;
    sqlx::query("UPDATE diraigent.tenant_member SET role='viewer' WHERE user_id=$1::uuid")
        .bind(account.json["user_id"].as_str().unwrap())
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        app.send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":agent})
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({})
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    app.cleanup().await;
}
