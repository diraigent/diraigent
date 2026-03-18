use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::OptionalAgentId;
use crate::error::AppError;
use crate::models::AgentContext;
use crate::services::embeddings::{cosine_similarity, top_k_from_env};

pub fn routes() -> Router<AppState> {
    Router::new().route("/agents/{agent_id}/context/{project_id}", get(get_context))
}

#[derive(Debug, Deserialize, Default)]
pub struct ContextQuery {
    /// When provided, embed the task's spec and return the top-k most relevant
    /// knowledge entries instead of the full list.
    pub task_id: Option<Uuid>,
}

/// Single call that loads everything an agent needs to operate on a project:
/// role, authorities, knowledge (scoped), decisions, integrations, ready tasks,
/// current tasks, open observations, recent events, and playbooks.
///
/// Pass `?task_id=<uuid>` to enable semantic ranking of knowledge entries by
/// cosine similarity to the task spec. Falls back to the full list when the
/// task has no spec or no embedding provider is configured.
async fn get_context(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id_header): OptionalAgentId,
    Path((agent_id, project_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ContextQuery>,
) -> Result<Json<AgentContext>, AppError> {
    // Verify the caller has access to this project (tenant isolation).
    crate::authz::require_membership(state.db.as_ref(), agent_id_header, user_id, project_id)
        .await?;

    let mut ctx = state
        .db
        .get_agent_context(agent_id, project_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Agent is not an active member of this project".into())
        })?;

    // Semantic ranking: when task_id is provided, replace the knowledge list
    // with the top-k entries most relevant to the task spec.
    if let Some(task_id) = query.task_id
        && let Ok(task) = state.db.get_task_by_id(task_id).await
    {
        // Build text to embed from the task spec (fall back to title).
        let spec_text = task
            .context
            .get("spec")
            .and_then(|s| s.as_str())
            .map(|s| format!("{}\n\n{}", task.title, s))
            .unwrap_or_else(|| task.title.clone());

        if let Ok(Some(query_vec)) = state.embedder.embed(&spec_text).await {
            // Fetch knowledge entries that have embeddings.
            let candidates = state
                .db
                .list_knowledge_with_embeddings(project_id)
                .await
                .unwrap_or_default();

            if !candidates.is_empty() {
                let top_k = top_k_from_env();

                // Score and rank by cosine similarity.
                let mut scored: Vec<(f64, _)> = candidates
                    .into_iter()
                    .filter_map(|k| {
                        let score = k
                            .embedding
                            .as_deref()
                            .map(|emb| cosine_similarity(&query_vec, emb))?;
                        Some((score, k))
                    })
                    .collect();

                scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                ctx.knowledge = scored.into_iter().take(top_k).map(|(_, k)| k).collect();
            }
            // If no candidates have embeddings yet, fall through to the
            // full list that get_agent_context already populated.
        }
    }

    Ok(Json(ctx))
}
