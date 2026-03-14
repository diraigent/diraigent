use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

pub async fn get_project_metrics(
    pool: &PgPool,
    project_id: Uuid,
    days: i32,
) -> Result<ProjectMetrics, AppError> {
    let since = Utc::now() - Duration::days(days as i64);

    let task_summary = get_task_summary(pool, project_id, since).await?;
    let tasks_per_day = get_tasks_per_day(pool, project_id, since).await?;
    let avg_time_in_state = get_avg_time_in_state(pool, project_id, since).await?;
    let agent_breakdown = get_agent_breakdown(pool, project_id, since).await?;
    let playbook_completion = get_playbook_completion(pool, project_id, since).await?;
    let cost_summary = get_cost_summary(pool, project_id, since).await?;
    let task_costs = get_task_costs(pool, project_id, since).await?;
    let tokens_per_day = get_tokens_per_day(pool, project_id, since).await?;

    Ok(ProjectMetrics {
        project_id,
        range_days: days,
        task_summary,
        tasks_per_day,
        avg_time_in_state_hours: avg_time_in_state,
        agent_breakdown,
        playbook_completion,
        cost_summary,
        task_costs,
        tokens_per_day,
    })
}

async fn get_task_summary(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<TaskSummary, AppError> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(
        "SELECT
            COUNT(*)::bigint AS total,
            COUNT(*) FILTER (WHERE state = 'done')::bigint AS done,
            COUNT(*) FILTER (WHERE state = 'cancelled')::bigint AS cancelled,
            COUNT(*) FILTER (WHERE state NOT IN ('backlog','ready','done','cancelled','human_review'))::bigint AS in_progress,
            COUNT(*) FILTER (WHERE state = 'ready')::bigint AS ready,
            COUNT(*) FILTER (WHERE state = 'backlog')::bigint AS backlog,
            COUNT(*) FILTER (WHERE state = 'human_review')::bigint AS human_review
         FROM diraigent.task
         WHERE project_id = $1 AND created_at >= $2",
    )
    .bind(project_id)
    .bind(since)
    .fetch_one(pool)
    .await?;

    Ok(TaskSummary {
        total: row.0,
        done: row.1,
        cancelled: row.2,
        in_progress: row.3,
        ready: row.4,
        backlog: row.5,
        human_review: row.6,
    })
}

async fn get_tasks_per_day(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<Vec<DayCount>, AppError> {
    let rows = sqlx::query_as::<_, DayCount>(
        "SELECT completed_at::date AS day, COUNT(*)::bigint AS count
         FROM diraigent.task
         WHERE project_id = $1 AND state = 'done' AND completed_at >= $2
         GROUP BY completed_at::date
         ORDER BY day",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn get_avg_time_in_state(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<Vec<StateAvg>, AppError> {
    // Compute LEAD() in a CTE first, then aggregate (window functions
    // cannot be nested inside aggregate functions in PostgreSQL).
    let rows = sqlx::query_as::<_, StateAvg>(
        "WITH transitions AS (
            SELECT
                al.action,
                al.entity_id,
                al.created_at,
                LEAD(al.created_at) OVER (
                    PARTITION BY al.entity_id ORDER BY al.created_at
                ) AS next_at,
                CASE WHEN t.state = 'done' THEN t.completed_at ELSE NOW() END AS fallback_end
            FROM diraigent.audit_log al
            JOIN diraigent.task t ON t.id = al.entity_id
            WHERE al.entity_type = 'task'
              AND al.action = 'task.transitioned'
              AND t.project_id = $1
              AND al.created_at >= $2
         )
         SELECT
            action AS state,
            AVG(EXTRACT(EPOCH FROM (
                COALESCE(next_at, fallback_end) - created_at
            )) / 3600)::float8 AS avg_hours
         FROM transitions
         GROUP BY action
         ORDER BY avg_hours DESC",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn get_agent_breakdown(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<Vec<AgentMetrics>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, String, i64, i64, Option<f64>)>(
        "SELECT
            a.id AS agent_id,
            a.name AS agent_name,
            COUNT(*) FILTER (WHERE t.state = 'done')::bigint AS tasks_completed,
            COUNT(*) FILTER (WHERE t.state NOT IN ('backlog','ready','done','cancelled'))::bigint AS tasks_in_progress,
            AVG(EXTRACT(EPOCH FROM (t.completed_at - t.claimed_at)) / 3600)
                FILTER (WHERE t.state = 'done' AND t.claimed_at IS NOT NULL AND t.completed_at IS NOT NULL)::float8
                AS avg_completion_hours
         FROM diraigent.agent a
         JOIN diraigent.task t ON t.assigned_agent_id = a.id
         WHERE t.project_id = $1 AND t.updated_at >= $2
         GROUP BY a.id, a.name
         ORDER BY tasks_completed DESC",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AgentMetrics {
            agent_id: r.0,
            agent_name: r.1,
            tasks_completed: r.2,
            tasks_in_progress: r.3,
            avg_completion_hours: r.4,
        })
        .collect())
}

async fn get_playbook_completion(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<Vec<PlaybookMetrics>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, String, i64, i64)>(
        "SELECT
            p.id AS playbook_id,
            p.title AS playbook_title,
            COUNT(t.id)::bigint AS total_tasks,
            COUNT(t.id) FILTER (WHERE t.state = 'done')::bigint AS completed_tasks
         FROM diraigent.playbook p
         JOIN diraigent.task t ON t.playbook_id = p.id
         WHERE t.project_id = $1 AND t.created_at >= $2
         GROUP BY p.id, p.title
         ORDER BY total_tasks DESC",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let rate = if r.2 > 0 {
                r.3 as f64 / r.2 as f64
            } else {
                0.0
            };
            PlaybookMetrics {
                playbook_id: r.0,
                playbook_title: r.1,
                total_tasks: r.2,
                completed_tasks: r.3,
                completion_rate: rate,
            }
        })
        .collect())
}

async fn get_cost_summary(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<CostSummary, AppError> {
    let row = sqlx::query_as::<_, (i64, i64, f64)>(
        "SELECT
            COALESCE(SUM(input_tokens),  0)::bigint           AS total_input_tokens,
            COALESCE(SUM(output_tokens), 0)::bigint           AS total_output_tokens,
            COALESCE(SUM(cost_usd),      0.0)::float8         AS total_cost_usd
         FROM diraigent.task
         WHERE project_id = $1 AND created_at >= $2",
    )
    .bind(project_id)
    .bind(since)
    .fetch_one(pool)
    .await?;

    Ok(CostSummary {
        total_input_tokens: row.0,
        total_output_tokens: row.1,
        total_cost_usd: row.2,
    })
}

async fn get_task_costs(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<Vec<TaskCostRow>, AppError> {
    let rows = sqlx::query_as::<_, TaskCostRow>(
        "SELECT
            id          AS task_id,
            number      AS task_number,
            title,
            state,
            input_tokens,
            output_tokens,
            cost_usd
         FROM diraigent.task
         WHERE project_id = $1
           AND created_at >= $2
           AND cost_usd > 0
         ORDER BY cost_usd DESC",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn get_tokens_per_day(
    pool: &PgPool,
    project_id: Uuid,
    since: chrono::DateTime<Utc>,
) -> Result<Vec<TokenDayCount>, AppError> {
    let rows = sqlx::query_as::<_, TokenDayCount>(
        "SELECT
            DATE(COALESCE(completed_at, updated_at)) AS day,
            SUM(input_tokens)::bigint                AS input_tokens,
            SUM(output_tokens)::bigint               AS output_tokens,
            SUM(cost_usd)::float8                    AS cost_usd
         FROM diraigent.task
         WHERE project_id = $1
           AND (completed_at >= $2 OR updated_at >= $2)
           AND (input_tokens > 0 OR output_tokens > 0)
         GROUP BY day
         ORDER BY day",
    )
    .bind(project_id)
    .bind(since)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
