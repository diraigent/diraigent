#[allow(dead_code, unused_macros)]
#[macro_use]
#[path = "integration/harness.rs"]
mod harness;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use harness::*;
use serde_json::json;
use uuid::Uuid;

fn workspace(mut req: Request<Body>, id: Uuid) -> Request<Body> {
    req.headers_mut()
        .insert("X-Tenant-Id", id.to_string().parse().unwrap());
    req
}

#[tokio::test]
async fn mixed_project_roles_do_not_inherit_power_from_another_workspace() {
    let app = TestApp::try_new()
        .await
        .expect("project access regression requires PostgreSQL");
    let a = app.create_project("Viewer project").await;
    let b = app.create_project("Editor project").await;
    let hidden = app.create_project("Hidden project").await;
    let ta = app.create_task(a, "Read this").await;
    let tb = app.create_task(b, "Edit this").await;
    let hidden_task = app.create_task(hidden, "Private task").await;
    let account = app.send(get("/v1/account")).await;
    let user: Uuid = account.json["user_id"].as_str().unwrap().parse().unwrap();
    let shared: Uuid = "00000000-0000-0000-0000-000000000001".parse().unwrap();
    let admin = Uuid::new_v4();
    sqlx::query("INSERT INTO diraigent.auth_user(user_id,auth_user_id) VALUES($1,'workspace-administrator')").bind(admin).execute(&app.pool).await.unwrap();
    sqlx::query("UPDATE diraigent.project SET owner_id=$1")
        .bind(admin)
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE diraigent.tenant_member SET role='member' WHERE user_id=$1")
        .bind(user)
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO diraigent.project_user_access(project_id,user_id,role) VALUES($1,$3,'viewer'),($2,$3,'editor')").bind(a).bind(b).bind(user).execute(&app.pool).await.unwrap();
    sqlx::query("INSERT INTO diraigent.task_dependency(task_id,depends_on) VALUES($1,$2)")
        .bind(ta["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .bind(hidden_task["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .execute(&app.pool)
        .await
        .unwrap();
    let deps = app
        .send(get(&format!(
            "/v1/tasks/{}/dependencies",
            ta["id"].as_str().unwrap()
        )))
        .await;
    assert_eq!(deps.status, StatusCode::OK);
    assert!(deps.json["depends_on"].as_array().unwrap().is_empty());
    assert_eq!(
        app.send(post_json(
            &format!("/v1/tasks/{}/dependencies", tb["id"].as_str().unwrap()),
            json!({"depends_on":hidden_task["id"]})
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let personal:Uuid=sqlx::query_scalar("INSERT INTO diraigent.tenant(name,slug) VALUES('Personal','personal-access-test') RETURNING id").fetch_one(&app.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO diraigent.tenant_member(tenant_id,user_id,role) VALUES($1,$2,'owner')",
    )
    .bind(personal)
    .bind(user)
    .execute(&app.pool)
    .await
    .unwrap();
    let tenants = app.send(get("/v1/tenants")).await;
    assert_eq!(tenants.json.as_array().unwrap().len(), 2);
    let selected = app.send(workspace(get("/v1/tenants/me"), personal)).await;
    assert_eq!(selected.json["id"], personal.to_string());
    let projects = app.send(workspace(get("/v1"), shared)).await;
    let ids: Vec<_> = projects
        .json
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert!(!ids.contains(&hidden.to_string().as_str()));
    let dashboard = app
        .send(workspace(get("/v1/dashboard/summary"), shared))
        .await;
    assert_eq!(dashboard.status, StatusCode::OK);
    assert_eq!(dashboard.json["projects"].as_array().unwrap().len(), 2);
    for id in [
        ta["id"].as_str().unwrap(),
        hidden_task["id"].as_str().unwrap(),
    ] {
        let response = app
            .send(workspace(
                put_json(&format!("/v1/tasks/{id}"), json!({"title":"Not allowed"})),
                shared,
            ))
            .await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }
    assert_eq!(
        app.send(workspace(
            get(&format!(
                "/v1/tasks/{}",
                hidden_task["id"].as_str().unwrap()
            )),
            shared
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(workspace(
            put_json(
                &format!("/v1/tasks/{}", tb["id"].as_str().unwrap()),
                json!({"title":"Allowed"})
            ),
            shared
        ))
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        app.send(workspace(
            put_json(&format!("/v1/{b}"), json!({"name":"Forbidden management"})),
            shared
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(workspace(
            post_json(
                &format!("/v1/{b}/people"),
                json!({"user_id":user,"role":"manager"})
            ),
            shared
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send(workspace(get("/v1/tenants/me"), Uuid::new_v4()))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let hidden_id = hidden_task["id"].as_str().unwrap();
    assert_eq!(
        app.send(workspace(
            post_json(
                "/v1/orchestra/sync",
                json!({"task_states":[{"task_id":hidden_id,"state":"done"}]})
            ),
            personal
        ))
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    for path in ["/v1/logs/labels", "/v1/settings"] {
        assert_eq!(
            app.send(workspace(get(path), personal)).await.status,
            StatusCode::FORBIDDEN
        );
    }
    for path in [
        format!("/v1/{a}/providers"),
        format!("/v1/{a}/integrations"),
        "/v1/account/export".into(),
        format!("/v1/tenants/{shared}/encryption/dek"),
    ] {
        assert_eq!(
            app.send(workspace(get(&path), shared)).await.status,
            StatusCode::FORBIDDEN
        );
    }
    app.cleanup().await;
}

#[tokio::test]
async fn manager_grants_only_existing_workspace_members_and_revocation_is_immediate() {
    let app = TestApp::try_new()
        .await
        .expect("project access regression requires PostgreSQL");
    let project = app.create_project("Shared").await;
    let member = Uuid::new_v4();
    sqlx::query("INSERT INTO diraigent.auth_user(user_id,auth_user_id) VALUES($1,'shared-member')")
        .bind(member)
        .execute(&app.pool)
        .await
        .unwrap();
    let path = format!("/v1/{project}/people");
    assert_eq!(
        app.send(post_json(&path, json!({"user_id":member,"role":"editor"})))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    sqlx::query("INSERT INTO diraigent.tenant_member(tenant_id,user_id,role) VALUES('00000000-0000-0000-0000-000000000001',$1,'member')").bind(member).execute(&app.pool).await.unwrap();
    assert_eq!(
        app.send(post_json(&path, json!({"user_id":member,"role":"manager"})))
            .await
            .status,
        StatusCode::OK
    );
    assert!(
        app.send(get(&path))
            .await
            .json
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["user_id"] == member.to_string())
    );
    assert_eq!(
        app.send(delete(&format!("{path}/{member}"))).await.status,
        StatusCode::OK
    );
    let role: Option<String> = sqlx::query_scalar("SELECT diraigent.human_project_role($1,$2)")
        .bind(member)
        .bind(project)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(role.is_none());
    app.send(post_json(&path, json!({"user_id":member,"role":"editor"})))
        .await;
    sqlx::query("DELETE FROM diraigent.tenant_member WHERE user_id=$1")
        .bind(member)
        .execute(&app.pool)
        .await
        .unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM diraigent.project_user_access WHERE user_id=$1")
            .bind(member)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    app.cleanup().await;
}

/// Distinct credentials exercise production authentication and immediate revocation.
#[tokio::test]
async fn independent_users_enforce_viewer_editor_manager_and_revocation() {
    let app = TestApp::try_new().await.expect("requires PostgreSQL");
    let project = app.create_project("Multi-user workspace").await;
    let task = app.create_task(project, "Shared task").await;
    let hidden = app.create_project("Another project").await;
    let shared_agent = app.create_agent("Shared workspace worker").await;
    let account = app.send(get("/v1/account")).await;
    let owner = account.json["user_id"].as_str().unwrap().parse().unwrap();
    let (hidden_agent, _) = diraigent_api::repository::register_agent(
        &app.pool,
        &diraigent_api::models::CreateAgent {
            name: "Worker outside the shared workspace".into(),
            capabilities: None,
            metadata: None,
        },
        owner,
    )
    .await
    .unwrap();
    let mut users = Vec::new();
    for role in ["viewer", "editor", "manager"] {
        let user = Uuid::new_v4();
        sqlx::query("INSERT INTO diraigent.auth_user(user_id,auth_user_id) VALUES($1,$2)")
            .bind(user)
            .bind(format!("test-{role}"))
            .execute(&app.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO diraigent.tenant_member(tenant_id,user_id,role) VALUES('00000000-0000-0000-0000-000000000001',$1,'member')").bind(user).execute(&app.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO diraigent.project_user_access(project_id,user_id,role) VALUES($1,$2,$3)",
        )
        .bind(project)
        .bind(user)
        .bind(role)
        .execute(&app.pool)
        .await
        .unwrap();
        let (_, key) = diraigent_api::repository::register_agent(
            &app.pool,
            &diraigent_api::models::CreateAgent {
                name: format!("credential-{role}"),
                capabilities: None,
                metadata: None,
            },
            user,
        )
        .await
        .unwrap();
        assert_eq!(
            app.send_authenticated(get(&format!("/v1/{project}/tasks")), &key)
                .await
                .status,
            StatusCode::OK
        );
        // Workspace members must discover workers without receiving administrative access.
        let agents = app.send_authenticated(get("/v1/agents"), &key).await;
        assert_eq!(
            agents.status,
            StatusCode::OK,
            "agent discovery for {role}: {}",
            agents.json
        );
        assert!(
            agents
                .json
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["name"] == format!("credential-{role}"))
        );
        assert!(
            !agents
                .json
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["id"] == hidden_agent.id.to_string())
        );
        assert!(
            agents
                .json
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["id"] == shared_agent.to_string())
        );
        assert_eq!(
            app.send_authenticated(workspace(get("/v1/agents"), Uuid::new_v4()), &key)
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.send_authenticated(post_json("/v1/agents/stream/ticket", json!(null)), &key)
                .await
                .status,
            if role == "viewer" {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::OK
            }
        );
        assert_eq!(
            app.send_authenticated(
                post_json("/v1/agents", json!({"name":"Unauthorized worker"})),
                &key
            )
            .await
            .status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.send_authenticated(get(&format!("/v1/{hidden}/tasks")), &key)
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        let edit = app
            .send_authenticated(
                put_json(
                    &format!("/v1/tasks/{}", task["id"].as_str().unwrap()),
                    json!({"title":format!("Edited by {role}")}),
                ),
                &key,
            )
            .await;
        assert_eq!(
            edit.status,
            if role == "viewer" {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::OK
            }
        );
        let manage = app
            .send_authenticated(
                put_json(
                    &format!("/v1/{project}"),
                    json!({"name":format!("Managed by {role}")}),
                ),
                &key,
            )
            .await;
        assert_eq!(
            manage.status,
            if role == "manager" {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        let own_workspace = app
            .send_authenticated(
                post_json("/v1/tenants", json!({"name":format!("Personal {role}")})),
                &key,
            )
            .await;
        assert_eq!(
            own_workspace.status,
            if role == "viewer" {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::OK
            }
        );
        // Creating a personal workspace cannot grant access to other shared projects.
        assert_eq!(
            app.send_authenticated(get(&format!("/v1/{hidden}/tasks")), &key)
                .await
                .status,
            StatusCode::FORBIDDEN
        );
        users.push((user, key));
    }
    let (viewer, viewer_key) = &users[0];
    let people = format!("/v1/{project}/people/{viewer}");
    assert_eq!(
        app.send_authenticated(delete(&people), &users[1].1)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.send_authenticated(delete(&people), &users[2].1)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.send_authenticated(get(&format!("/v1/{project}/tasks")), viewer_key)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    app.cleanup().await;
}
