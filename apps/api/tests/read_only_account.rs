#[allow(dead_code, unused_macros)]
#[macro_use]
#[path = "integration/harness.rs"]
mod harness;

use axum::http::StatusCode;
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
