use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::projects::get_project_by_id;
use super::{Table, delete_by_id, fetch_by_id};

// ── Work ──

pub async fn create_work(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateWork,
    created_by: Uuid,
) -> Result<Work, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let success_criteria = req
        .success_criteria
        .clone()
        .unwrap_or(serde_json::json!([]));
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));
    let work_type = req.work_type.as_deref().unwrap_or("epic");
    let priority = req.priority.unwrap_or(0);
    let auto_status = req.auto_status.unwrap_or(false);

    let work = sqlx::query_as::<_, Work>(
        "INSERT INTO diraigent.work (project_id, title, description, work_type, priority, parent_work_id, auto_status, intent_type, success_criteria, metadata, created_by, sort_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                 (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM diraigent.work WHERE project_id = $1))
         RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(work_type)
    .bind(priority)
    .bind(req.parent_work_id)
    .bind(auto_status)
    .bind(&req.intent_type)
    .bind(&success_criteria)
    .bind(&metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(work)
}

pub async fn get_work_by_id(pool: &PgPool, id: Uuid) -> Result<Work, AppError> {
    fetch_by_id(pool, Table::Work, id, "Work not found").await
}

pub async fn activate_work(pool: &PgPool, work_id: Uuid) -> Result<Work, AppError> {
    // Try to activate: only from 'active' or 'paused'
    let maybe = sqlx::query_as::<_, Work>(
        "UPDATE diraigent.work SET status = 'ready', updated_at = now()
         WHERE id = $1 AND status IN ('active', 'paused')
         RETURNING *",
    )
    .bind(work_id)
    .fetch_optional(pool)
    .await?;

    if let Some(work) = maybe {
        return Ok(work);
    }

    // No rows affected — check why
    let existing = get_work_by_id(pool, work_id).await?; // 404 if not found
    match existing.status.as_str() {
        "ready" | "processing" => Err(AppError::Conflict(format!(
            "Work item is already {}",
            existing.status
        ))),
        "achieved" | "abandoned" => Err(AppError::Validation(format!(
            "Cannot activate a work item with status '{}'",
            existing.status
        ))),
        _ => Err(AppError::Validation(format!(
            "Cannot activate a work item with status '{}'",
            existing.status
        ))),
    }
}

pub async fn list_works(
    pool: &PgPool,
    project_id: Uuid,
    filters: &WorkFilters,
) -> Result<Vec<Work>, AppError> {
    let limit = filters.limit.unwrap_or(200).min(500);
    let offset = filters.offset.unwrap_or(0);
    let top_level = filters.top_level.unwrap_or(false);

    let exclude_statuses: Vec<String> = filters
        .status_not
        .as_deref()
        .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
        .unwrap_or_default();

    let works = sqlx::query_as::<_, Work>(
        "SELECT * FROM diraigent.work
         WHERE project_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR work_type = $3)
           AND ($4::uuid IS NULL OR parent_work_id = $4)
           AND (NOT $5 OR parent_work_id IS NULL)
           AND (cardinality($8::text[]) = 0 OR status != ALL($8))
         ORDER BY sort_order ASC, created_at DESC
         LIMIT $6 OFFSET $7",
    )
    .bind(project_id)
    .bind(&filters.status)
    .bind(&filters.work_type)
    .bind(filters.parent_work_id)
    .bind(top_level)
    .bind(limit)
    .bind(offset)
    .bind(&exclude_statuses)
    .fetch_all(pool)
    .await?;

    Ok(works)
}

pub async fn work_status_counts(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<(String, i64)>, AppError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT status, COUNT(*) FROM diraigent.work
         WHERE project_id = $1
         GROUP BY status",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn update_work(pool: &PgPool, id: Uuid, req: &UpdateWork) -> Result<Work, AppError> {
    let existing = get_work_by_id(pool, id).await?;

    // Self-parent check
    if let Some(Some(parent_id)) = req.parent_work_id
        && parent_id == id
    {
        return Err(AppError::Validation(
            "A work item cannot be its own parent".into(),
        ));
    }

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let description = req
        .description
        .as_deref()
        .or(existing.description.as_deref());
    let status = req.status.as_deref().unwrap_or(&existing.status);
    let work_type = req.work_type.as_deref().unwrap_or(&existing.work_type);
    let priority = req.priority.unwrap_or(existing.priority);
    let auto_status = req.auto_status.unwrap_or(existing.auto_status);
    let parent_work_id = match req.parent_work_id {
        None => existing.parent_work_id, // no change
        Some(None) => None,              // clear
        Some(Some(pid)) => Some(pid),    // set
    };
    let intent_type = match &req.intent_type {
        None => existing.intent_type.as_deref(), // no change
        Some(None) => None,                      // clear
        Some(Some(v)) => Some(v.as_str()),       // set
    };
    let success_criteria = req
        .success_criteria
        .as_ref()
        .unwrap_or(&existing.success_criteria);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);

    let work = sqlx::query_as::<_, Work>(
        "UPDATE diraigent.work
         SET title = $2, description = $3, status = $4, work_type = $5, priority = $6,
             parent_work_id = $7, auto_status = $8, intent_type = $9,
             success_criteria = $10, metadata = $11, sort_order = $12
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(work_type)
    .bind(priority)
    .bind(parent_work_id)
    .bind(auto_status)
    .bind(intent_type)
    .bind(success_criteria)
    .bind(metadata)
    .bind(sort_order)
    .fetch_one(pool)
    .await?;

    Ok(work)
}

pub async fn delete_work(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Work, id, "Work not found").await
}

pub async fn link_task_work(
    pool: &PgPool,
    work_id: Uuid,
    task_id: Uuid,
) -> Result<TaskWork, AppError> {
    let tw = sqlx::query_as::<_, TaskWork>(
        "INSERT INTO diraigent.task_work (task_id, work_id, position)
         VALUES ($1, $2, (SELECT COALESCE(MAX(position), 0) + 1 FROM diraigent.task_work WHERE work_id = $2))
         RETURNING *",
    )
    .bind(task_id)
    .bind(work_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            AppError::Conflict("Link already exists or invalid reference".into())
        }
        _ => e.into(),
    })?;
    Ok(tw)
}

pub async fn unlink_task_work(pool: &PgPool, work_id: Uuid, task_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM diraigent.task_work WHERE work_id = $1 AND task_id = $2")
        .bind(work_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Link not found".into()));
    }
    Ok(())
}

pub async fn get_work_progress(pool: &PgPool, work_id: Uuid) -> Result<WorkProgress, AppError> {
    let _ = get_work_by_id(pool, work_id).await?;

    let row = sqlx::query_as::<_, (i64, i64)>(
        "WITH RECURSIVE descendants AS (
            SELECT id FROM diraigent.work WHERE id = $1
            UNION ALL
            SELECT w.id FROM diraigent.work w JOIN descendants d ON w.parent_work_id = d.id
        )
        SELECT
            COUNT(DISTINCT tw.task_id)::bigint,
            COUNT(DISTINCT tw.task_id) FILTER (WHERE t.state = 'done')::bigint
         FROM diraigent.task_work tw
         JOIN diraigent.task t ON t.id = tw.task_id
         WHERE tw.work_id IN (SELECT id FROM descendants)",
    )
    .bind(work_id)
    .fetch_one(pool)
    .await?;

    Ok(WorkProgress {
        work_id,
        total_tasks: row.0,
        done_tasks: row.1,
    })
}

pub async fn get_work_stats(pool: &PgPool, work_id: Uuid) -> Result<WorkStats, AppError> {
    let _ = get_work_by_id(pool, work_id).await?;

    let row = sqlx::query_as::<_, (
        i64, i64, i64, i64, i64, i64,
        Option<serde_json::Value>,
        Option<f64>, i64, i64,
        i64,
        Option<f64>,
        Option<chrono::DateTime<chrono::Utc>>,
    )>(
        "WITH tasks AS (
            SELECT t.* FROM diraigent.task_work tw
            JOIN diraigent.task t ON t.id = tw.task_id
            WHERE tw.work_id = $1
        ),
        blocked AS (
            SELECT DISTINCT td.task_id FROM diraigent.task_dependency td
            JOIN tasks t ON td.task_id = t.id
            JOIN diraigent.task blocker ON td.depends_on = blocker.id
            WHERE blocker.state != 'done'
        ),
        kind_agg AS (
            SELECT COALESCE(jsonb_object_agg(kind, cnt), '{}'::jsonb) AS breakdown
            FROM (SELECT kind, COUNT(*)::bigint AS cnt FROM tasks GROUP BY kind) k
        )
        SELECT
            COUNT(*) FILTER (WHERE state = 'backlog')::bigint AS backlog_count,
            COUNT(*) FILTER (WHERE state = 'ready')::bigint AS ready_count,
            COUNT(*) FILTER (WHERE state NOT IN ('backlog','ready','done','cancelled'))::bigint AS working_count,
            COUNT(*) FILTER (WHERE state = 'done')::bigint AS done_count,
            COUNT(*) FILTER (WHERE state = 'cancelled')::bigint AS cancelled_count,
            COUNT(*)::bigint AS total_count,
            (SELECT breakdown FROM kind_agg) AS kind_breakdown,
            SUM(cost_usd)::double precision AS total_cost_usd,
            COALESCE(SUM(input_tokens)::bigint, 0) AS total_input_tokens,
            COALESCE(SUM(output_tokens)::bigint, 0) AS total_output_tokens,
            (SELECT COUNT(*) FROM blocked)::bigint AS blocked_count,
            EXTRACT(EPOCH FROM AVG(completed_at - created_at)
                FILTER (WHERE completed_at IS NOT NULL))::double precision / 3600.0 AS avg_completion_hours,
            MIN(created_at) FILTER (WHERE state NOT IN ('done','cancelled')) AS oldest_open_task_date
        FROM tasks",
    )
    .bind(work_id)
    .fetch_one(pool)
    .await?;

    Ok(WorkStats {
        work_id,
        backlog_count: row.0,
        ready_count: row.1,
        working_count: row.2,
        done_count: row.3,
        cancelled_count: row.4,
        total_count: row.5,
        kind_breakdown: row.6.unwrap_or(serde_json::json!({})),
        total_cost_usd: row.7.unwrap_or(0.0),
        total_input_tokens: row.8,
        total_output_tokens: row.9,
        blocked_count: row.10,
        avg_completion_hours: row.11,
        oldest_open_task_date: row.12,
    })
}

pub async fn compute_auto_status(pool: &PgPool, work_id: Uuid) -> Result<Option<String>, AppError> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE t.state = 'done') AS done,
            COUNT(*) FILTER (WHERE t.state = 'cancelled') AS cancelled,
            COUNT(*) FILTER (WHERE t.state NOT IN ('backlog','ready','done','cancelled')) AS working
         FROM diraigent.task_work tw
         JOIN diraigent.task t ON t.id = tw.task_id
         WHERE tw.work_id = $1",
    )
    .bind(work_id)
    .fetch_one(pool)
    .await?;

    let (total, done, cancelled, working) = row;
    if total == 0 {
        return Ok(None);
    }
    if done == total {
        return Ok(Some("achieved".to_string()));
    }
    if cancelled == total {
        return Ok(Some("abandoned".to_string()));
    }
    if working > 0 {
        return Ok(Some("active".to_string()));
    }
    // All backlog/ready or mix of done/cancelled but not all one
    Ok(Some("active".to_string()))
}

pub async fn list_work_tasks(
    pool: &PgPool,
    work_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Task>, AppError> {
    let _ = get_work_by_id(pool, work_id).await?;
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT t.* FROM diraigent.task t
         JOIN diraigent.task_work tw ON t.id = tw.task_id
         WHERE tw.work_id = $1
         ORDER BY tw.position ASC, t.urgent DESC, t.created_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(work_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(tasks)
}

pub async fn count_work_tasks(pool: &PgPool, work_id: Uuid) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM diraigent.task_work WHERE work_id = $1")
        .bind(work_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn bulk_link_tasks(
    pool: &PgPool,
    work_id: Uuid,
    task_ids: &[Uuid],
) -> Result<i64, AppError> {
    let _ = get_work_by_id(pool, work_id).await?;
    let result = sqlx::query(
        "INSERT INTO diraigent.task_work (task_id, work_id)
         SELECT unnest($2::uuid[]), $1
         ON CONFLICT DO NOTHING",
    )
    .bind(work_id)
    .bind(task_ids)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() as i64)
}

pub async fn reorder_work_tasks(
    pool: &PgPool,
    work_id: Uuid,
    task_ids: &[Uuid],
) -> Result<Vec<Task>, AppError> {
    if task_ids.is_empty() {
        return Ok(vec![]);
    }

    // Validate all task_ids are linked to this work item
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM diraigent.task_work WHERE work_id = $1 AND task_id = ANY($2)",
    )
    .bind(work_id)
    .bind(task_ids)
    .fetch_one(pool)
    .await?;

    if count.0 != task_ids.len() as i64 {
        return Err(AppError::Validation(
            "Some task IDs are not linked to this work item".into(),
        ));
    }

    let indexes: Vec<i32> = (0..task_ids.len() as i32).collect();

    sqlx::query(
        "UPDATE diraigent.task_work SET position = data.new_pos
         FROM (SELECT unnest($1::uuid[]) AS tid, unnest($2::int[]) AS new_pos) data
         WHERE diraigent.task_work.work_id = $3 AND diraigent.task_work.task_id = data.tid",
    )
    .bind(task_ids)
    .bind(&indexes)
    .bind(work_id)
    .execute(pool)
    .await?;

    // Return tasks in new order
    let tasks = sqlx::query_as::<_, Task>(
        "SELECT t.* FROM diraigent.task t
         JOIN diraigent.task_work tw ON t.id = tw.task_id
         WHERE tw.work_id = $1
         ORDER BY tw.position ASC",
    )
    .bind(work_id)
    .fetch_all(pool)
    .await?;

    Ok(tasks)
}

// ── Work Comments ──

pub async fn create_work_comment(
    pool: &PgPool,
    work_id: Uuid,
    req: &CreateWorkComment,
    user_id: Option<Uuid>,
) -> Result<WorkComment, AppError> {
    let metadata = req
        .metadata
        .clone()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let comment = sqlx::query_as::<_, WorkComment>(
        "INSERT INTO diraigent.work_comment (work_id, agent_id, user_id, content, metadata)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(work_id)
    .bind(req.agent_id)
    .bind(user_id)
    .bind(&req.content)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(comment)
}

pub async fn list_work_comments(
    pool: &PgPool,
    work_id: Uuid,
    p: &Pagination,
) -> Result<Vec<WorkComment>, AppError> {
    let limit = p.limit.unwrap_or(50).min(100);
    let offset = p.offset.unwrap_or(0);

    let comments = sqlx::query_as::<_, WorkComment>(
        "SELECT * FROM diraigent.work_comment WHERE work_id = $1
         ORDER BY created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(work_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(comments)
}

pub async fn list_works_for_task(pool: &PgPool, task_id: Uuid) -> Result<Vec<Work>, AppError> {
    let works = sqlx::query_as::<_, Work>(
        "SELECT w.* FROM diraigent.work w
         JOIN diraigent.task_work tw ON tw.work_id = w.id
         WHERE tw.task_id = $1
         ORDER BY w.sort_order ASC, w.created_at ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(works)
}

pub async fn list_auto_status_work_ids_for_task(
    pool: &PgPool,
    task_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_as::<_, (Uuid,)>(
        "SELECT w.id FROM diraigent.work w
         JOIN diraigent.task_work tw ON tw.work_id = w.id
         WHERE tw.task_id = $1 AND w.auto_status = true",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(|r| r.0).collect())
}

pub async fn reorder_works(
    pool: &PgPool,
    project_id: Uuid,
    work_ids: &[Uuid],
) -> Result<Vec<Work>, AppError> {
    if work_ids.is_empty() {
        return Ok(vec![]);
    }

    // Validate all work_ids belong to the given project
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM diraigent.work WHERE id = ANY($1) AND project_id = $2",
    )
    .bind(work_ids)
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    if count.0 != work_ids.len() as i64 {
        return Err(AppError::Validation(
            "Some work IDs do not belong to this project".into(),
        ));
    }

    // Build sort_order updates: work_ids[i] gets sort_order = i
    let indexes: Vec<i32> = (0..work_ids.len() as i32).collect();

    sqlx::query(
        "UPDATE diraigent.work SET sort_order = data.new_order
         FROM (SELECT unnest($1::uuid[]) AS id, unnest($2::int[]) AS new_order) data
         WHERE diraigent.work.id = data.id",
    )
    .bind(work_ids)
    .bind(&indexes)
    .execute(pool)
    .await?;

    // Return the updated work items in new sort order
    let works = sqlx::query_as::<_, Work>(
        "SELECT * FROM diraigent.work WHERE project_id = $1 ORDER BY sort_order ASC, created_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(works)
}

/// Return all work IDs linked to a task (no auto_status filter).
pub async fn get_work_ids_for_task(pool: &PgPool, task_id: Uuid) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_as::<_, (Uuid,)>(
        "SELECT tw.work_id FROM diraigent.task_work tw WHERE tw.task_id = $1",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(|r| r.0).collect())
}

/// Return distinct work IDs from all active (non-done/cancelled) tasks
/// assigned to the given agent in the given project, excluding a specific task.
/// Used to inherit work associations when an agent creates subtasks.
pub async fn get_agent_inherited_work_ids(
    pool: &PgPool,
    agent_id: Uuid,
    project_id: Uuid,
    exclude_task_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_as::<_, (Uuid,)>(
        "SELECT DISTINCT tw.work_id
         FROM diraigent.task_work tw
         JOIN diraigent.task t ON t.id = tw.task_id
         WHERE t.assigned_agent_id = $1
           AND t.project_id = $2
           AND t.state NOT IN ('done', 'cancelled')
           AND t.id != $3",
    )
    .bind(agent_id)
    .bind(project_id)
    .bind(exclude_task_id)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(|r| r.0).collect())
}
