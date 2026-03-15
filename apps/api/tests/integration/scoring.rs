use crate::harness::*;
use axum::http::StatusCode;

/// Integration test: create 3 tasks with different scoring profiles and verify
/// the ready-tasks endpoint returns them in composite score order, not just FIFO.
///
/// Task profiles:
/// 1. High-priority fresh: urgent=true, linked to priority-1 work item
/// 2. Medium-priority stale: not urgent, but with created_at pushed back in time
/// 3. Low-priority critical-path: not urgent, fresh, but blocks another task
#[tokio::test]
async fn ready_tasks_ordered_by_composite_score() {
    let app = require_db!();
    let project_id = app.create_project("scoring-test").await;

    // ── Create 3 ready tasks ──

    // Task 1: urgent, linked to high-priority work
    let task1 = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Urgent high-priority",
                "urgent": true,
            }),
        )
        .await;
    let task1_id = task1["id"].as_str().unwrap();

    // Task 2: not urgent, will be made stale via direct DB update
    let task2 = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Stale medium task",
            }),
        )
        .await;
    let task2_id = task2["id"].as_str().unwrap();

    // Task 3: not urgent, fresh, will block another task (critical path)
    let task3 = app
        .create_task_with(
            project_id,
            serde_json::json!({
                "title": "Critical path blocker",
            }),
        )
        .await;
    let task3_id = task3["id"].as_str().unwrap();

    // ── Transition all to ready ──
    for tid in [task1_id, task2_id, task3_id] {
        let resp = app
            .send(post_json(
                &format!("/v1/tasks/{tid}/transition"),
                serde_json::json!({ "state": "ready" }),
            ))
            .await;
        assert_eq!(
            resp.status,
            StatusCode::OK,
            "transition {tid}: {}",
            resp.json
        );
    }

    // ── Make task2 stale: set created_at to 20 days ago ──
    sqlx::query("UPDATE diraigent.task SET created_at = NOW() - INTERVAL '20 days' WHERE id = $1")
        .bind(uuid::Uuid::parse_str(task2_id).unwrap())
        .execute(&app.pool)
        .await
        .expect("Failed to backdate task2");

    // ── Make task3 a critical-path blocker: create a dependent task ──
    let dependent = app
        .create_task_with(
            project_id,
            serde_json::json!({ "title": "Blocked by task3" }),
        )
        .await;
    let dep_id = dependent["id"].as_str().unwrap();
    // Add dependency: dependent depends on task3
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{dep_id}/dependencies"),
            serde_json::json!({ "depends_on": task3_id }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "add dep: {}", resp.json);

    // ── Create a work item and link task1 to it ──
    let work_resp = app
        .send(post_json(
            &format!("/v1/{project_id}/work"),
            serde_json::json!({
                "title": "Top priority work",
                "priority": 1,
            }),
        ))
        .await;
    assert_eq!(
        work_resp.status,
        StatusCode::OK,
        "create work: {}",
        work_resp.json
    );
    let work_id = work_resp.json["id"].as_str().unwrap();

    // Link task1 to the work item
    let link_resp = app
        .send(post_json(
            &format!("/v1/work/{work_id}/tasks"),
            serde_json::json!({ "task_ids": [task1_id] }),
        ))
        .await;
    assert_eq!(
        link_resp.status,
        StatusCode::OK,
        "link task to work: {}",
        link_resp.json
    );

    // ── Fetch ready tasks and verify ordering ──
    let resp = app
        .send(get(&format!("/v1/{project_id}/tasks/ready")))
        .await;
    assert_eq!(resp.status, StatusCode::OK, "list ready: {}", resp.json);

    let tasks = resp
        .json
        .as_array()
        .expect("expected array of scored tasks");
    assert!(
        tasks.len() >= 3,
        "expected at least 3 ready tasks, got {}",
        tasks.len()
    );

    // All tasks should have a score field
    for t in tasks {
        assert!(
            t.get("score").is_some(),
            "task {} missing score field",
            t["title"]
        );
        let score = t["score"].as_f64().unwrap();
        assert!(score >= 0.0, "score should be non-negative");
    }

    // Verify score-based ordering: each task's score >= next task's score
    for i in 0..tasks.len() - 1 {
        let s1 = tasks[i]["score"].as_f64().unwrap();
        let s2 = tasks[i + 1]["score"].as_f64().unwrap();
        assert!(
            s1 >= s2,
            "Tasks not in score-descending order: task '{}' (score={}) before task '{}' (score={})",
            tasks[i]["title"],
            s1,
            tasks[i + 1]["title"],
            s2,
        );
    }

    // Find our 3 tasks in the results
    let find_score = |title: &str| -> f64 {
        tasks
            .iter()
            .find(|t| t["title"].as_str().unwrap() == title)
            .unwrap_or_else(|| panic!("task '{title}' not found in results"))["score"]
            .as_f64()
            .unwrap()
    };

    let score_urgent = find_score("Urgent high-priority");
    let score_stale = find_score("Stale medium task");
    let score_critical = find_score("Critical path blocker");

    // Stale task (20 days age) should outscore the critical-path task (1 blocking * 1.5)
    // stale: ~20.0 age, critical: ~1.5 dep
    assert!(
        score_stale > score_critical,
        "Stale task ({score_stale}) should outscore critical path ({score_critical})"
    );

    // Urgent + work-linked task should have the highest score
    // urgent: 10.0 urgent + 5.0 work = 15.0 (fresh)
    // But stale is 20 days = 20.0, so stale may be higher
    // The important thing: manual priority alone doesn't determine order
    // The urgent+work task and the stale task should both outscore the fresh+blocking task
    assert!(
        score_urgent > score_critical,
        "Urgent task ({score_urgent}) should outscore critical path ({score_critical})"
    );

    // Verify score_components is present
    let first_task = &tasks[0];
    assert!(
        first_task.get("score_components").is_some(),
        "score_components should be present"
    );
    let components = &first_task["score_components"];
    assert!(
        components.get("age_score").is_some(),
        "age_score component missing"
    );
    assert!(
        components.get("urgent_score").is_some(),
        "urgent_score component missing"
    );
    assert!(
        components.get("dependency_score").is_some(),
        "dependency_score component missing"
    );
    assert!(
        components.get("work_score").is_some(),
        "work_score component missing"
    );
    assert!(components.get("total").is_some(), "total component missing");

    app.cleanup().await;
}

/// Test that score_components sum to the total score.
#[tokio::test]
async fn score_components_sum_to_total() {
    let app = require_db!();
    let project_id = app.create_project("scoring-sum-test").await;

    let task = app.create_task(project_id, "Sum test task").await;
    let task_id = task["id"].as_str().unwrap();

    // Transition to ready
    let resp = app
        .send(post_json(
            &format!("/v1/tasks/{task_id}/transition"),
            serde_json::json!({ "state": "ready" }),
        ))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    // Fetch ready tasks
    let resp = app
        .send(get(&format!("/v1/{project_id}/tasks/ready")))
        .await;
    assert_eq!(resp.status, StatusCode::OK);

    let tasks = resp.json.as_array().unwrap();
    assert!(!tasks.is_empty());

    for t in tasks {
        let score = t["score"].as_f64().unwrap();
        let components = &t["score_components"];
        let age = components["age_score"].as_f64().unwrap();
        let urgent = components["urgent_score"].as_f64().unwrap();
        let dep = components["dependency_score"].as_f64().unwrap();
        let work = components["work_score"].as_f64().unwrap();
        let total = components["total"].as_f64().unwrap();

        let component_sum = age + urgent + dep + work;
        assert!(
            (component_sum - total).abs() < 0.01,
            "Components sum ({component_sum}) != total ({total})"
        );
        assert!(
            (score - total).abs() < 0.01,
            "score field ({score}) != total ({total})"
        );
    }

    app.cleanup().await;
}
