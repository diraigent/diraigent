//! Context fetching helpers for the orchestra.
//!
//! Centralises calls to the agent context endpoint and provides
//! task-scoped variants that pass `?task_id=<uuid>` for semantic
//! knowledge ranking.

use anyhow::Result;
use serde_json::Value;

use crate::api::ProjectsApi;
use crate::crypto::Dek;

/// Fetch the full agent context for a project.
///
/// When `task_id` is `Some`, the context endpoint returns the top-k knowledge
/// entries most relevant to the task spec via cosine similarity. Falls back to
/// the full list when the embedding service is unavailable or task has no spec.
pub async fn fetch_context(
    api: &ProjectsApi,
    project_id: &str,
    task_id: Option<&str>,
    dek: Option<&Dek>,
) -> Result<Value> {
    let mut ctx = match task_id {
        Some(tid) => api.get_context_for_task(project_id, tid).await?,
        None => api.get_context(project_id).await?,
    };

    if let Some(dek) = dek {
        crate::crypto::decrypt_json_recursive(dek, &mut ctx, "context");
    }

    Ok(ctx)
}

/// Like [`fetch_context`] but returns an empty JSON object on error
/// instead of propagating the error. Used in prompt building where a
/// missing context should degrade gracefully rather than abort the task.
pub async fn fetch_context_or_empty(
    api: &ProjectsApi,
    project_id: &str,
    task_id: Option<&str>,
    dek: Option<&Dek>,
) -> Value {
    fetch_context(api, project_id, task_id, dek)
        .await
        .unwrap_or(Value::Object(serde_json::Map::new()))
}
