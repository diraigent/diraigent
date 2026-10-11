#[allow(dead_code, unused_macros)]
#[macro_use]
#[path = "integration/harness.rs"]
mod harness;

use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::STANDARD};
use harness::*;
use serde_json::json;

#[tokio::test]
async fn viewer_browses_normal_routes_but_cannot_escape_or_execute() {
    let app = TestApp::try_new()
        .await
        .expect("read-only regression requires PostgreSQL");
    let project = app.create_project("Demo").await;
    let task = app.create_task(project, "Demo task").await;
    let account = app.send(get("/v1/account")).await;
    let user = account.json["user_id"].as_str().unwrap();
    sqlx::query("UPDATE diraigent.tenant_member SET role = 'viewer' WHERE user_id = $1::uuid")
        .bind(user)
        .execute(&app.pool)
        .await
        .unwrap();

    let account = app.send(get("/v1/account")).await;
    assert_eq!(account.status, StatusCode::OK);
    assert_eq!(account.json["read_only"], true);
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(
        app.send(get(&format!("/v1/tasks/{task_id}"))).await.status,
        StatusCode::OK
    );
    for path in [
        "/v1".to_string(),
        "/v1/tenants".into(),
        "/v1/agents".into(),
        format!("/v1/{project}/tasks"),
        format!("/v1/{project}/chat"),
        format!("/v1/tasks/{task_id}/transition"),
        format!("/v1/{project}/git/push-main"),
        "/v1/review/stream/ticket".into(),
    ] {
        assert_eq!(
            app.send(post_json(&path, json!({}))).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    assert_eq!(
        app.send(delete("/v1/account")).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(put_json(
            &format!("/v1/tasks/{task_id}"),
            json!({"title":"Changed"})
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(get("/v1/account/export")).await.status,
        StatusCode::FORBIDDEN
    );
    // Removing the restriction takes effect immediately without a JWT/cache refresh.
    sqlx::query("UPDATE diraigent.tenant_member SET role = 'owner' WHERE user_id = $1::uuid")
        .bind(user)
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(app.send(get("/v1/account")).await.json["read_only"], false);
    assert_eq!(
        app.send(put_json(
            &format!("/v1/tasks/{task_id}"),
            json!({"title":"Changed"})
        ))
        .await
        .status,
        StatusCode::OK
    );
    app.cleanup().await;
}

#[tokio::test]
async fn viewer_projection_preserves_views_and_stored_audit_and_diff_data() {
    let app = TestApp::try_new().await.expect("requires PostgreSQL");
    let project = app.create_project("Redacted demo").await;
    let private = "http://192.168.7.19:3000 /Users/test/private/project password=private-password user@example.test";
    let task = app.create_task_with(project, json!({"title":"Visible task","context":{"spec":private,"api_key":"private-key","files":["src/app.rs"]}})).await;
    let task_id = task["id"].as_str().unwrap();
    let log = app
        .send(post_json(
            &format!("/v1/{project}/task-logs"),
            json!({"task_id":task_id,"step_name":"implement","content":private}),
        ))
        .await;
    assert_eq!(log.status, StatusCode::OK);
    let log_id = log.json["id"].as_str().unwrap();
    let file:uuid::Uuid=sqlx::query_scalar("INSERT INTO diraigent.task_changed_file(task_id,path,change_type,diff) VALUES($1::uuid,'src/app.rs','modified',$2) RETURNING id").bind(task_id).bind(format!("+{private}")).fetch_one(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO diraigent.audit_log(project_id,action,entity_type,entity_id,summary,before_state) VALUES($1,'task.updated','task',$2::uuid,$3,$4)").bind(project).bind(task_id).bind(private).bind(json!({"context":{"spec":private,"password":"private-password"}})).execute(&app.pool).await.unwrap();
    let account = app.send(get("/v1/account")).await;
    let user = account.json["user_id"].as_str().unwrap();
    sqlx::query("UPDATE diraigent.tenant_member SET role='viewer' WHERE user_id=$1::uuid")
        .bind(user)
        .execute(&app.pool)
        .await
        .unwrap();
    for path in [
        format!("/v1/tasks/{task_id}"),
        format!("/v1/tasks/{task_id}/changed-files/{file}"),
        format!("/v1/{project}/audit"),
        format!("/v1/task-logs/{log_id}"),
    ] {
        let response = app.send(get(&path)).await;
        assert_eq!(response.status, StatusCode::OK, "{path}");
        let output = response.json.to_string();
        for secret in [
            "192.168.7.19",
            "/Users/test",
            "private-password",
            "private-key",
            "user@example.test",
        ] {
            assert!(!output.contains(secret), "{path} leaked {secret}");
        }
        assert!(output.contains("[redacted]"));
    }
    let mut head = get(&format!("/v1/tasks/{task_id}"));
    *head.method_mut() = axum::http::Method::HEAD;
    assert_eq!(app.send(head).await.status, StatusCode::OK);
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT context FROM diraigent.task WHERE id=$1::uuid")
            .bind(task_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(stored["spec"], private);
    sqlx::query("UPDATE diraigent.tenant_member SET role='owner' WHERE user_id=$1::uuid")
        .bind(user)
        .execute(&app.pool)
        .await
        .unwrap();
    let original = app.send(get(&format!("/v1/tasks/{task_id}"))).await;
    assert_eq!(original.json["context"]["spec"], private);
    app.cleanup().await;
}

#[tokio::test]
async fn real_worker_key_cannot_bypass_viewer_and_source_projection() {
    let app = TestApp::try_new().await.expect("requires PostgreSQL");
    let project = app.create_project("Shared demo").await;
    let task = app.create_task(project, "Visible task").await;
    let hidden = app.create_project("Not shared").await;
    let hidden_task = app.create_task(hidden, "Hidden task").await;
    let agent = app.create_agent("Source worker").await;
    sqlx::query("UPDATE diraigent.project SET git_mode='monorepo' WHERE id=$1")
        .bind(project)
        .execute(&app.pool)
        .await
        .unwrap();
    let viewer = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO diraigent.auth_user(user_id,auth_user_id) VALUES($1,'demo-regression-viewer')",
    )
    .bind(viewer)
    .execute(&app.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO diraigent.tenant_member(tenant_id,user_id,role) VALUES('00000000-0000-0000-0000-000000000001',$1,'viewer')")
        .bind(viewer).execute(&app.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO diraigent.project_user_access(project_id,user_id,role) VALUES($1,$2,'viewer')",
    )
    .bind(project)
    .bind(viewer)
    .execute(&app.pool)
    .await
    .unwrap();
    let (_, key) = diraigent_api::repository::register_agent(
        &app.pool,
        &diraigent_api::models::CreateAgent {
            name: "Previously issued viewer key".into(),
            capabilities: None,
            metadata: None,
        },
        viewer,
    )
    .await
    .unwrap();
    let registry = app.ws_registry.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    registry.register(agent, tx);
    let responder = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if let diraigent_api::ws_protocol::WsMessage::GitRequest {
                request_id,
                query_type,
                ..
            } = message
            {
                let data = match query_type.as_str() {
                    "source_tree" => {
                        json!({"entries":[{"name":"app.rs","path":"src/app.rs","kind":"file"}]})
                    }
                    "source_blob" => {
                        let content = "// public code\npassword=\"two secret words\"\nhttp://100.100.1.2:8082 worker.local";
                        json!({"content":STANDARD.encode(content),"encoding":"base64","size":content.len()})
                    }
                    _ => panic!("Viewer triggered unexpected worker operation: {query_type}"),
                };
                registry.complete_git_request(
                    &request_id,
                    diraigent_api::ws_registry::GitResponsePayload {
                        success: true,
                        error: None,
                        data,
                    },
                );
            }
        }
    });
    assert_eq!(
        app.send_authenticated(
            get(&format!("/v1/tasks/{}", task["id"].as_str().unwrap())),
            &key
        )
        .await
        .status,
        StatusCode::OK
    );
    let list = app.send_authenticated(get("/v1"), &key).await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.json.as_array().unwrap().len(), 1);
    for path in [
        format!("/v1/{hidden}"),
        format!("/v1/tasks/{}", hidden_task["id"].as_str().unwrap()),
        "/v1/settings".into(),
        "/v1/logs".into(),
    ] {
        assert_eq!(
            app.send_authenticated(get(&path), &key).await.status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    for path in [
        "/v1/agents".to_owned(),
        "/v1/tenants".into(),
        format!("/v1/{project}/chat"),
        format!("/v1/{project}/git/push-main"),
    ] {
        assert_eq!(
            app.send_authenticated(post_json(&path, json!({})), &key)
                .await
                .status,
            StatusCode::FORBIDDEN,
            "{path}"
        );
    }
    let tree = app
        .send_authenticated(get(&format!("/v1/{project}/source/tree")), &key)
        .await;
    assert_eq!(tree.status, StatusCode::OK);
    assert_eq!(tree.json["entries"][0]["path"], "src/app.rs");
    let blob = app
        .send_authenticated(
            get(&format!("/v1/{project}/source/blob?path=src/app.rs")),
            &key,
        )
        .await;
    assert_eq!(blob.status, StatusCode::OK);
    let clean = String::from_utf8(
        STANDARD
            .decode(blob.json["content"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(clean.contains("public code"));
    assert!(clean.contains("[redacted]"));
    for private in ["two secret words", "100.100.1.2", "worker.local"] {
        assert!(!clean.contains(private));
    }
    responder.abort();
    app.cleanup().await;
}
