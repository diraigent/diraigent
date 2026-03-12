use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::{Table, fetch_by_id};

// ── File Locks ──

pub async fn list_file_locks(pool: &PgPool, project_id: Uuid) -> Result<Vec<FileLock>, AppError> {
    let locks = sqlx::query_as::<_, FileLock>(
        "SELECT * FROM diraigent.file_lock WHERE project_id = $1 ORDER BY locked_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    Ok(locks)
}

pub async fn acquire_file_locks(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
    paths: &[String],
    agent_id: Uuid,
) -> Result<Vec<FileLock>, AppError> {
    // Fetch existing locks for this project (excluding locks owned by the same task)
    let existing = sqlx::query_as::<_, FileLock>(
        "SELECT * FROM diraigent.file_lock WHERE project_id = $1 AND task_id != $2",
    )
    .bind(project_id)
    .bind(task_id)
    .fetch_all(pool)
    .await?;

    // Check for overlaps between requested paths and existing locks
    for requested in paths {
        for held in &existing {
            if globs_overlap(requested, &held.path_glob) {
                return Err(AppError::Conflict(format!(
                    "Path '{}' conflicts with existing lock '{}' held by task {}",
                    requested, held.path_glob, held.task_id
                )));
            }
        }
    }

    // Insert all locks
    let mut locks = Vec::with_capacity(paths.len());
    for path in paths {
        let lock = sqlx::query_as::<_, FileLock>(
            "INSERT INTO diraigent.file_lock (project_id, task_id, path_glob, locked_by)
             VALUES ($1, $2, $3, $4)
             RETURNING *",
        )
        .bind(project_id)
        .bind(task_id)
        .bind(path)
        .bind(agent_id)
        .fetch_one(pool)
        .await?;
        locks.push(lock);
    }

    Ok(locks)
}

pub async fn release_file_locks(
    pool: &PgPool,
    project_id: Uuid,
    task_id: Uuid,
) -> Result<u64, AppError> {
    let result =
        sqlx::query("DELETE FROM diraigent.file_lock WHERE project_id = $1 AND task_id = $2")
            .bind(project_id)
            .bind(task_id)
            .execute(pool)
            .await?;

    Ok(result.rows_affected())
}

/// Release all file locks for a task (by task_id only, used during state transitions).
pub async fn release_file_locks_for_task(pool: &PgPool, task_id: Uuid) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM diraigent.file_lock WHERE task_id = $1")
        .bind(task_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Check if two glob patterns could match overlapping file paths.
///
/// Rules:
/// - `**` matches any number of path segments
/// - `*` matches anything within a single segment
/// - Exact segments must match literally
/// - If either pattern is a prefix of the other (considering `**`), they overlap
fn globs_overlap(a: &str, b: &str) -> bool {
    let a_parts: Vec<&str> = a.split('/').filter(|s| !s.is_empty()).collect();
    let b_parts: Vec<&str> = b.split('/').filter(|s| !s.is_empty()).collect();
    segments_overlap(&a_parts, &b_parts)
}

fn segments_overlap(a: &[&str], b: &[&str]) -> bool {
    // Base cases
    if a.is_empty() && b.is_empty() {
        return true;
    }
    if a.is_empty() {
        // a is exhausted — overlaps only if b starts with ** (which matches zero segments)
        return b.first() == Some(&"**") && segments_overlap(a, &b[1..]);
    }
    if b.is_empty() {
        return a.first() == Some(&"**") && segments_overlap(&a[1..], b);
    }

    let a0 = a[0];
    let b0 = b[0];

    // ** in a: can match zero or more segments of b
    if a0 == "**" {
        // Try matching zero segments (skip the **), or consume one segment of b
        return segments_overlap(&a[1..], b) || segments_overlap(a, &b[1..]);
    }
    // ** in b: symmetric
    if b0 == "**" {
        return segments_overlap(a, &b[1..]) || segments_overlap(&a[1..], b);
    }

    // Both are concrete segments — check if they could match the same filename
    if segment_matches(a0, b0) {
        return segments_overlap(&a[1..], &b[1..]);
    }

    false
}

/// Check if two glob segments could match at least one common string.
fn segment_matches(a: &str, b: &str) -> bool {
    // If either is a wildcard, it matches anything in a single segment
    if a == "*" || b == "*" {
        return true;
    }
    // Simple literal comparison (covers most real-world cases)
    a == b
}

// ── Changed Files ──

pub async fn create_changed_files(
    pool: &PgPool,
    task_id: Uuid,
    req: &CreateChangedFiles,
) -> Result<Vec<ChangedFileSummary>, AppError> {
    // Clear existing entries so repeated calls are idempotent
    sqlx::query("DELETE FROM diraigent.task_changed_file WHERE task_id = $1")
        .bind(task_id)
        .execute(pool)
        .await?;

    let mut results = Vec::with_capacity(req.files.len());
    for f in &req.files {
        let row = sqlx::query_as::<_, ChangedFileSummary>(
            "INSERT INTO diraigent.task_changed_file (task_id, path, change_type, diff)
             VALUES ($1, $2, $3, $4)
             RETURNING id, task_id, path, change_type, created_at",
        )
        .bind(task_id)
        .bind(&f.path)
        .bind(&f.change_type)
        .bind(&f.diff)
        .fetch_one(pool)
        .await?;
        results.push(row);
    }
    Ok(results)
}

pub async fn list_changed_files(
    pool: &PgPool,
    task_id: Uuid,
) -> Result<Vec<ChangedFileSummary>, AppError> {
    let files = sqlx::query_as::<_, ChangedFileSummary>(
        "SELECT id, task_id, path, change_type, created_at
         FROM diraigent.task_changed_file
         WHERE task_id = $1
         ORDER BY path",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(files)
}

pub async fn get_changed_file_by_id(pool: &PgPool, id: Uuid) -> Result<ChangedFile, AppError> {
    fetch_by_id(pool, Table::ChangedFile, id, "Changed file not found").await
}
