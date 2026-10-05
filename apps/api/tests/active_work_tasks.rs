//! Run against a disposable PostgreSQL database with:
//! DATABASE_URL=... cargo test -p diraigent-api --test active_work_tasks -- --ignored
use diraigent_api::{migration_runner, models::*, repository};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(pool: &PgPool) -> (Uuid, Uuid) {
    migration_runner::run(pool).await.unwrap();
    let user = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO diraigent.auth_user (auth_user_id) VALUES ('work-test') RETURNING user_id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let tenant = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO diraigent.tenant (name, slug) VALUES ('Test', 'work-test') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let project = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO diraigent.project (name, slug, owner_id, tenant_id, package_id)
         VALUES ('Test', 'work-test', $1, $2,
                 (SELECT id FROM diraigent.package WHERE slug = 'software-dev')) RETURNING id",
    )
    .bind(user)
    .bind(tenant)
    .fetch_one(pool)
    .await
    .unwrap();
    (project, user)
}

async fn work(pool: &PgPool, project: Uuid, user: Uuid, status: &str) -> Work {
    let req: CreateWork = serde_json::from_value(json!({"title": "Work"})).unwrap();
    let work = repository::create_work(pool, project, &req, user)
        .await
        .unwrap();
    set_work_status(pool, work.id, status).await
}

async fn set_work_status(pool: &PgPool, id: Uuid, status: &str) -> Work {
    let req: UpdateWork = serde_json::from_value(json!({"status": status})).unwrap();
    repository::update_work(pool, id, &req).await.unwrap()
}

async fn task(pool: &PgPool, project: Uuid, user: Uuid) -> Task {
    let req: CreateTask = serde_json::from_value(json!({"title": "Task"})).unwrap();
    repository::create_task(pool, project, &req, user)
        .await
        .unwrap()
}

async fn assert_state(pool: &PgPool, id: Uuid, expected: &str) {
    assert_eq!(
        repository::get_task_by_id(pool, id).await.unwrap().state,
        expected
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "requires disposable PostgreSQL DATABASE_URL"]
async fn starting_work_promotes_only_its_backlog_tasks(pool: PgPool) {
    let (project, user) = fixture(&pool).await;
    let work = work(&pool, project, user, "paused").await;
    let unlinked = task(&pool, project, user).await;
    let mut linked = vec![];
    for state in [
        "backlog",
        "ready",
        "working",
        "human_review",
        "done",
        "cancelled",
    ] {
        let task = task(&pool, project, user).await;
        sqlx::query("UPDATE diraigent.task SET state = $2, state_entered_at = now() - interval '1 day' WHERE id = $1")
            .bind(task.id).bind(state).execute(&pool).await.unwrap();
        repository::link_task_work(&pool, work.id, task.id)
            .await
            .unwrap();
        linked.push((task.id, state));
    }

    set_work_status(&pool, work.id, "active").await;
    for (id, old) in linked {
        assert_state(&pool, id, if old == "backlog" { "ready" } else { old }).await;
        let task = repository::get_task_by_id(&pool, id).await.unwrap();
        if old == "backlog" {
            assert!(task.state_entered_at > chrono::Utc::now() - chrono::Duration::minutes(1));
        } else {
            assert!(task.state_entered_at < chrono::Utc::now() - chrono::Duration::hours(23));
        }
    }
    assert_state(&pool, unlinked.id, "backlog").await;
}

#[sqlx::test(migrations = false)]
#[ignore = "requires disposable PostgreSQL DATABASE_URL"]
async fn links_to_started_work_queue_tasks_but_not_duplicate_links(pool: PgPool) {
    let (project, user) = fixture(&pool).await;
    for status in [
        "active",
        "ready",
        "processing",
        "paused",
        "achieved",
        "abandoned",
    ] {
        let work = work(&pool, project, user, status).await;
        let single = task(&pool, project, user).await;
        let bulk = task(&pool, project, user).await;
        repository::link_task_work(&pool, work.id, single.id)
            .await
            .unwrap();
        assert_eq!(
            repository::bulk_link_tasks(&pool, work.id, &[bulk.id])
                .await
                .unwrap(),
            1
        );
        let expected = if matches!(status, "active" | "ready" | "processing") {
            "ready"
        } else {
            "backlog"
        };
        assert_state(&pool, single.id, expected).await;
        assert_state(&pool, bulk.id, expected).await;

        if expected == "ready" {
            repository::transition_task(&pool, single.id, "backlog")
                .await
                .unwrap();
            repository::transition_task(&pool, bulk.id, "backlog")
                .await
                .unwrap();
        }
        assert!(
            repository::link_task_work(&pool, work.id, single.id)
                .await
                .is_err()
        );
        assert_eq!(
            repository::bulk_link_tasks(&pool, work.id, &[single.id, bulk.id])
                .await
                .unwrap(),
            0
        );
        assert_state(&pool, single.id, "backlog").await;
        assert_state(&pool, bulk.id, "backlog").await;
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "requires disposable PostgreSQL DATABASE_URL"]
async fn manual_backlog_survives_work_updates_and_new_links(pool: PgPool) {
    let (project, user) = fixture(&pool).await;
    let work = work(&pool, project, user, "active").await;
    let held = task(&pool, project, user).await;
    repository::link_task_work(&pool, work.id, held.id)
        .await
        .unwrap();
    repository::transition_task(&pool, held.id, "backlog")
        .await
        .unwrap();
    let req: UpdateWork =
        serde_json::from_value(json!({"title": "Edited", "metadata": {"note": "edit"}})).unwrap();
    repository::update_work(&pool, work.id, &req).await.unwrap();
    set_work_status(&pool, work.id, "active").await;
    let new = task(&pool, project, user).await;
    repository::bulk_link_tasks(&pool, work.id, &[held.id, new.id])
        .await
        .unwrap();
    assert_state(&pool, held.id, "backlog").await;
    assert_state(&pool, new.id, "ready").await;

    // Explicit start/resume is a new scheduling event.
    set_work_status(&pool, work.id, "paused").await;
    repository::activate_work(&pool, work.id).await.unwrap();
    assert_state(&pool, held.id, "ready").await;
    assert_eq!(
        repository::get_work_by_id(&pool, work.id)
            .await
            .unwrap()
            .status,
        "ready"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "requires disposable PostgreSQL DATABASE_URL"]
async fn queued_tasks_still_respect_dependencies(pool: PgPool) {
    let (project, user) = fixture(&pool).await;
    let work = work(&pool, project, user, "active").await;
    let blocker = task(&pool, project, user).await;
    let blocked = task(&pool, project, user).await;
    repository::add_dependency(&pool, blocked.id, blocker.id)
        .await
        .unwrap();
    repository::link_task_work(&pool, work.id, blocked.id)
        .await
        .unwrap();
    assert_state(&pool, blocked.id, "ready").await;
    assert!(
        repository::list_ready_tasks(&pool, project, 100, 0)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repository::claim_task(&pool, blocked.id, Uuid::now_v7())
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "requires disposable PostgreSQL DATABASE_URL"]
async fn linking_and_starting_concurrently_do_not_miss_a_task(pool: PgPool) {
    let (project, user) = fixture(&pool).await;
    let work = work(&pool, project, user, "paused").await;
    let task = task(&pool, project, user).await;
    let (started, linked) = tokio::join!(
        repository::activate_work(&pool, work.id),
        repository::link_task_work(&pool, work.id, task.id),
    );
    started.unwrap();
    linked.unwrap();
    assert_state(&pool, task.id, "ready").await;
}
