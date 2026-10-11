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

async fn enable_heavy(app: &TestApp, project: Uuid) -> (Uuid, Payloads) {
    let agent = app.create_agent("Full storage owner").await;
    let values = connect_protocol(app, agent, Uuid::new_v4(), 2);
    let response = app
        .send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":agent,"heavy_content":true}),
        ))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    (agent, values)
}

#[tokio::test]
async fn heavy_fields_round_trip_without_central_bodies_and_partial_updates_preserve_references() {
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Full storage").await;
    let (agent, values) = enable_heavy(&app, project).await;
    let task = app.send(post_json(&format!("/v1/{project}/tasks"),json!({"title":"Task","context":{"spec":"private-spec","worker":{"provider":"opencode"}}}))).await;
    assert_eq!(task.status, StatusCode::OK, "{}", task.json);
    assert_eq!(task.json["context"]["spec"], "private-spec");
    let id = task.id();
    let raw: Value = sqlx::query_scalar("SELECT context FROM diraigent.task WHERE id=$1")
        .bind(id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(!raw.to_string().contains("private-spec"));
    assert_eq!(raw["worker"]["provider"], "opencode");
    let updated = app
        .send(put_json(
            &format!("/v1/tasks/{id}"),
            json!({"title":"Renamed"}),
        ))
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.json);
    assert_eq!(updated.json["context"], task.json["context"]);
    let knowledge = app
        .send(post_json(
            &format!("/v1/{project}/knowledge"),
            json!({"title":"Guide","category":"pattern","content":"private-knowledge"}),
        ))
        .await;
    assert_eq!(knowledge.status, StatusCode::OK, "{}", knowledge.json);
    assert_eq!(
        app.send(get(&format!("/v1/knowledge/{}", knowledge.id())))
            .await
            .json["content"],
        "private-knowledge"
    );
    let comment = app
        .send(post_json(
            &format!("/v1/tasks/{id}/comments"),
            json!({"content":"private-comment"}),
        ))
        .await;
    assert_eq!(comment.status, StatusCode::OK, "{}", comment.json);
    let event=app.send(post_json(&format!("/v1/{project}/events"),json!({"title":"Signal","kind":"custom","source":"test","description":"private-event","metadata":{"large":"X".repeat(2048)}}))).await;
    assert_eq!(event.status, StatusCode::OK, "{}", event.json);
    assert_eq!(event.json["description"], "private-event");
    let ctx = app
        .send(get(&format!("/v1/agents/{agent}/context/{project}")))
        .await;
    assert_eq!(ctx.status, StatusCode::OK, "{}", ctx.json);
    assert_eq!(ctx.json["knowledge"][0]["content"], "private-knowledge");
    assert_eq!(ctx.json["recent_events"][0]["description"], "private-event");
    let raw: String = sqlx::query_scalar("SELECT content FROM diraigent.knowledge WHERE id=$1")
        .bind(knowledge.id())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(!raw.contains("private-knowledge"));
    assert!(raw.starts_with(FIELD_PREFIX));
    assert!(
        values
            .lock()
            .unwrap()
            .keys()
            .any(|(_, kind, _)| kind == "field")
    );
}

#[tokio::test]
async fn structured_content_and_embedding_migrate_without_changing_public_shapes() {
    use diraigent_api::db::{DiraigentDb, PostgresDb};
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Structured fields").await;
    enable_heavy(&app, project).await;
    for (collection, body, field, expected) in [
        (
            "decisions",
            json!({"title":"Choice","context":"why","alternatives":[{"name":"A","pros":"private pros"}],"consequences":"private consequences"}),
            "alternatives",
            json!([{"name":"A","pros":"private pros"}]),
        ),
        (
            "work",
            json!({"title":"Plan","description":"private plan","success_criteria":["private acceptance"]}),
            "success_criteria",
            json!(["private acceptance"]),
        ),
        (
            "observations",
            json!({"title":"Finding","description":"private finding","evidence":{"detail":"private evidence"}}),
            "evidence",
            json!({"detail":"private evidence"}),
        ),
        (
            "verifications",
            json!({"title":"Check","kind":"test","detail":"private check","evidence":{"detail":"private output"}}),
            "evidence",
            json!({"detail":"private output"}),
        ),
    ] {
        let created = app
            .send(post_json(&format!("/v1/{project}/{collection}"), body))
            .await;
        assert_eq!(
            created.status,
            StatusCode::OK,
            "{collection}: {}",
            created.json
        );
        assert_eq!(created.json[field], expected, "{collection}");
        let table = match collection {
            "decisions" => "decision",
            "work" => "work",
            "observations" => "observation",
            _ => "verification",
        };
        let raw: Value = sqlx::query_scalar(&format!(
            "SELECT to_jsonb(t) FROM diraigent.{table} t WHERE id=$1"
        ))
        .bind(created.id())
        .fetch_one(&app.pool)
        .await
        .unwrap();
        assert!(!raw.to_string().contains("private"), "{collection}: {raw}");
        if collection == "observations" {
            let promoted = app
                .send(post_json(
                    &format!("/v1/observations/{}/promote", created.id()),
                    json!({}),
                ))
                .await;
            assert_eq!(promoted.status, StatusCode::OK, "{}", promoted.json);
            assert_eq!(promoted.json["work"]["description"], "private finding");
        }
    }
    let task = app.create_task(project, "Local sync").await;
    let update = Uuid::new_v4();
    let synced = app.send(post_json("/v1/orchestra/sync",json!({"task_updates":[{"id":update,"task_id":task["id"],"kind":"progress","content":"private synced progress"}]}))).await;
    assert_eq!(synced.status, StatusCode::OK, "{}", synced.json);
    let central: String =
        sqlx::query_scalar("SELECT content FROM diraigent.task_update WHERE id=$1")
            .bind(update)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(central.starts_with(FIELD_PREFIX));
    let updates = app
        .send(get(&format!(
            "/v1/tasks/{}/updates",
            task["id"].as_str().unwrap()
        )))
        .await;
    assert_eq!(updates.json[0]["content"], "private synced progress");
    let knowledge = app
        .send(post_json(
            &format!("/v1/{project}/knowledge"),
            json!({"title":"Vector","content":"source","category":"pattern"}),
        ))
        .await;
    let db = PostgresDb::new(app.pool.clone()).with_content(app.ws_registry.clone());
    db.update_knowledge_embedding(knowledge.id(), &[1.0, 0.0])
        .await
        .unwrap();
    let central: Option<Vec<f64>> =
        sqlx::query_scalar("SELECT embedding FROM diraigent.knowledge WHERE id=$1")
            .bind(knowledge.id())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(central.is_none());
    let entries = db.list_knowledge_with_embeddings(project).await.unwrap();
    assert_eq!(entries[0].embedding, Some(vec![1.0, 0.0]));
    assert_eq!(entries[0].content, "source");
}

#[tokio::test]
async fn legacy_heavy_migration_is_verified_restartable_and_retains_read_shapes() {
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Legacy fields").await;
    let task = app
        .send(post_json(
            &format!("/v1/{project}/tasks"),
            json!({"title":"Legacy","context":{"spec":"legacy-spec"}}),
        ))
        .await;
    let knowledge = app
        .send(post_json(
            &format!("/v1/{project}/knowledge"),
            json!({"title":"Legacy guide","category":"pattern","content":"legacy-body"}),
        ))
        .await;
    let event=app.send(post_json(&format!("/v1/{project}/events"),json!({"title":"Legacy signal","kind":"custom","source":"test","description":"legacy-event"}))).await;
    let (_, values) = enable_heavy(&app, project).await;
    let first = app
        .send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({}),
        ))
        .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.json);
    assert!(first.json["moved"].as_u64().unwrap() > 0);
    // Background audit writes may complete between batches; drain bounded work.
    for _ in 0..5 {
        let batch = app
            .send(post_json(
                &format!("/v1/{project}/storage/migrate"),
                json!({}),
            ))
            .await;
        assert_eq!(batch.status, StatusCode::OK, "{}", batch.json);
    }
    let last = app
        .send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({}),
        ))
        .await;
    assert_eq!(last.json["moved"], 0, "{}", last.json);
    assert_eq!(
        app.send(get(&format!("/v1/tasks/{}", task.id())))
            .await
            .json["context"]["spec"],
        "legacy-spec"
    );
    assert_eq!(
        app.send(get(&format!("/v1/knowledge/{}", knowledge.id())))
            .await
            .json["content"],
        "legacy-body"
    );
    assert_eq!(
        app.send(get(&format!("/v1/events/{}", event.id())))
            .await
            .json["description"],
        "legacy-event"
    );
    assert!(
        values
            .lock()
            .unwrap()
            .values()
            .any(|v| v == &json!("legacy-body"))
    );
    let raw: String = sqlx::query_scalar("SELECT description FROM diraigent.event WHERE id=$1")
        .bind(event.id())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(raw.starts_with(FIELD_PREFIX));
}

#[tokio::test]
async fn full_storage_requires_protocol_two_and_fails_closed_on_corruption_and_disconnect() {
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Guarded content").await;
    let old = app.create_agent("Old protocol").await;
    connect(&app, old, Uuid::new_v4());
    let denied = app
        .send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":old,"heavy_content":true}),
        ))
        .await;
    assert_eq!(denied.status, StatusCode::BAD_REQUEST);
    let (agent, values) = enable_heavy(&app, project).await;
    let task = app
        .send(post_json(
            &format!("/v1/{project}/tasks"),
            json!({"title":"Corruptible","context":{"spec":"original"}}),
        ))
        .await;
    assert_eq!(task.status, StatusCode::OK, "{}", task.json);
    let key = values
        .lock()
        .unwrap()
        .iter()
        .find(|(_, v)| *v == &json!({"spec":"original"}))
        .unwrap()
        .0
        .to_owned();
    values
        .lock()
        .unwrap()
        .insert(key, json!({"spec":"corrupted"}));
    assert_eq!(
        app.send(get(&format!("/v1/tasks/{}", task.id())))
            .await
            .status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    app.ws_registry.unregister(agent);
    let failed = app
        .send(post_json(
            &format!("/v1/{project}/knowledge"),
            json!({"title":"Offline","category":"pattern","content":"never-persist-this"}),
        ))
        .await;
    assert_eq!(failed.status, StatusCode::SERVICE_UNAVAILABLE);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM diraigent.knowledge WHERE project_id=$1")
            .bind(project)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
#[tokio::test]
async fn legacy_export_does_not_overwrite_an_edit_made_during_transfer() {
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Migration race").await;
    let created = app
        .send(post_json(
            &format!("/v1/{project}/knowledge"),
            json!({"title":"Guide","content":"legacy racing content","category":"pattern"}),
        ))
        .await;
    let agent = app.create_agent("Racing owner").await;
    connect_with_edit(&app, agent, Uuid::new_v4(), 2, Some(created.id()));
    assert_eq!(
        app.send(put_json(
            &format!("/v1/{project}/storage"),
            json!({"agent_id":agent,"heavy_content":true})
        ))
        .await
        .status,
        StatusCode::OK
    );
    let batch = app
        .send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({}),
        ))
        .await;
    assert_eq!(batch.status, StatusCode::OK, "{}", batch.json);
    let content: String = sqlx::query_scalar("SELECT content FROM diraigent.knowledge WHERE id=$1")
        .bind(created.id())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(content, "concurrent edit");
    assert_eq!(
        app.send(post_json(
            &format!("/v1/{project}/storage/migrate"),
            json!({})
        ))
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        app.send(get(&format!("/v1/knowledge/{}", created.id())))
            .await
            .json["content"],
        "concurrent edit"
    );
}

#[tokio::test]
async fn nonmembers_are_denied_before_owner_access_and_publication_hydrates_only_published_content()
{
    let app = TestApp::try_new()
        .await
        .expect("storage regressions require PostgreSQL");
    let project = app.create_project("Publication").await;
    let (agent, _) = enable_heavy(&app, project).await;
    let task = app
        .send(post_json(
            &format!("/v1/{project}/tasks"),
            json!({"title":"Published task","context":{"spec":"published specification"}}),
        ))
        .await;
    let knowledge=app.send(post_json(&format!("/v1/{project}/knowledge"),json!({"title":"Published guide","content":"published guide content","category":"pattern"}))).await;
    assert_eq!(
        app.send(get(&format!(
            "/v1/spectator/projects/{project}/tasks/{}",
            task.id()
        )))
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    sqlx::query("UPDATE diraigent.project SET metadata=metadata || '{\"spectator_enabled\":true}'::jsonb WHERE id=$1").bind(project).execute(&app.pool).await.unwrap();
    let public = app
        .send(get(&format!(
            "/v1/spectator/projects/{project}/tasks/{}",
            task.id()
        )))
        .await;
    assert_eq!(public.status, StatusCode::OK, "{}", public.json);
    assert_eq!(public.json["spec"], "published specification");
    let public = app
        .send(get(&format!(
            "/v1/spectator/projects/{project}/knowledge/{}",
            knowledge.id()
        )))
        .await;
    assert_eq!(public.json["content"], "published guide content");
    assert_eq!(
        app.send(get(&format!("/v1/{project}/search?q=guide")))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.send(get(&format!("/v1/tasks/{}/related", task.id())))
            .await
            .status,
        StatusCode::OK
    );
    let account = app.send(get("/v1/account")).await;
    let user: Uuid = account.json["user_id"].as_str().unwrap().parse().unwrap();
    let owner = Uuid::new_v4();
    sqlx::query("INSERT INTO diraigent.auth_user(user_id,auth_user_id) VALUES($1,'other-owner')")
        .bind(owner)
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE diraigent.project SET owner_id=$1 WHERE id=$2")
        .bind(owner)
        .bind(project)
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE diraigent.tenant_member SET role='member' WHERE user_id=$1")
        .bind(user)
        .execute(&app.pool)
        .await
        .unwrap();
    app.ws_registry.unregister(agent);
    // An owner request would now fail with 503; authorization must return 403 first.
    assert_eq!(
        app.send(get(&format!("/v1/tasks/{}", task.id())))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(get(&format!("/v1/knowledge/{}", knowledge.id())))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

fn connect(app: &TestApp, agent: Uuid, store_id: Uuid) -> Payloads {
    connect_protocol(app, agent, store_id, 1)
}
fn connect_protocol(app: &TestApp, agent: Uuid, store_id: Uuid, protocol: u64) -> Payloads {
    connect_with_edit(app, agent, store_id, protocol, None)
}
fn connect_with_edit(
    app: &TestApp,
    agent: Uuid,
    store_id: Uuid,
    protocol: u64,
    edit: Option<Uuid>,
) -> Payloads {
    let pool = app.pool.clone();
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
                    ContentRequest::Probe => Ok(json!({"protocol":protocol,"store_id":store_id})),
                    ContentRequest::Put { kind, id, value } => {
                        if value == json!("legacy racing content")
                            && let Some(knowledge) = edit
                        {
                            sqlx::query("UPDATE diraigent.knowledge SET content='concurrent edit' WHERE id=$1").bind(knowledge).execute(&pool).await.unwrap();
                        }
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
                    ContentRequest::SearchFields { .. } => Ok(json!({"data":[],"total":0})),
                    ContentRequest::RankFields { .. } => Ok(json!([])),
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
