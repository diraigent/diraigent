use crate::harness::*;
use axum::http::StatusCode;

#[tokio::test]
async fn retirement_preserves_tasks_and_requires_review_for_legacy_work() {
    let app = require_db!();
    let project = app.create_project("retirement").await;
    let direct = app.create_task(project, "Direct").await;
    let legacy = app.create_task(project, "Legacy queued step").await;
    let review = app.create_task(project, "Legacy review").await;
    let done = app.create_task(project, "Previously finished").await;
    // Restore the pre-047 columns in this isolated fixture, then apply exactly
    // the committed forward retirement SQL. No historical migration is changed.
    sqlx::raw_sql(
        "ALTER TABLE diraigent.task DROP CONSTRAINT task_state_check;
        ALTER TABLE diraigent.task ADD COLUMN playbook_name text;
        ALTER TABLE diraigent.task ADD COLUMN playbook_step integer;
        ALTER TABLE diraigent.project ADD COLUMN default_playbook_name text;",
    )
    .execute(&app.pool)
    .await
    .unwrap();
    for (task, state, book) in [
        (&direct, "ready", None),
        (&legacy, "ready", Some("standard")),
        (&review, "wait:review", Some("standard")),
        (&done, "done", Some("standard")),
    ] {
        sqlx::query(
            "UPDATE diraigent.task SET state=$2, playbook_name=$3, playbook_step=1 WHERE id=$1",
        )
        .bind(task["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap())
        .bind(state)
        .bind(book)
        .execute(&app.pool)
        .await
        .unwrap();
    }
    sqlx::raw_sql(include_str!("../../migrations/047_retire_playbooks.sql"))
        .execute(&app.pool)
        .await
        .unwrap();
    for (task, expected) in [
        (&direct, "ready"),
        (&legacy, "human_review"),
        (&review, "human_review"),
        (&done, "done"),
    ] {
        let id = task["id"].as_str().unwrap();
        let response = app.send(get(&format!("/v1/tasks/{id}"))).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json["state"], expected);
        assert!(response.json.get("playbook_name").is_none());
    }
    // Retired endpoints are absent, not a partially operational workflow API.
    for path in ["/v1/playbooks", "/v1/step-templates"] {
        assert!(matches!(
            app.send(get(path)).await.status,
            StatusCode::NOT_FOUND | StatusCode::BAD_REQUEST
        ));
    }
    // Re-running the complete migrator validates original checksums unchanged.
    diraigent_api::migration_runner::run(&app.pool)
        .await
        .unwrap();
    app.cleanup().await;
}

#[tokio::test]
async fn direct_claim_is_exclusive_and_rejects_stage_states() {
    let app = require_db!();
    let project = app.create_project("direct-lifecycle").await;
    let task = app.create_task(project, "Run directly").await;
    let id = task["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap();
    let first_agent = app.create_agent("first").await;
    let second_agent = app.create_agent("second").await;
    diraigent_api::repository::transition_task(&app.pool, id, "ready")
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        diraigent_api::repository::claim_task(&app.pool, id, first_agent),
        diraigent_api::repository::claim_task(&app.pool, id, second_agent)
    );
    assert_ne!(first.is_ok(), second.is_ok());
    for state in ["implement", "review", "wait:review"] {
        assert!(
            diraigent_api::repository::transition_task(&app.pool, id, state)
                .await
                .is_err()
        );
    }
    let completed = diraigent_api::repository::transition_task(&app.pool, id, "done")
        .await
        .unwrap();
    assert_eq!(completed.state, "done");
    assert!(completed.completed_at.is_some());
    app.cleanup().await;
}
