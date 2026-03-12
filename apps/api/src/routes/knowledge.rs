use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{
    OptionalAgentId, ensure_authority_on, ensure_member, require_authority, require_membership,
};
use crate::error::AppError;
use crate::models::*;
use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/knowledge", post(create).get(list))
        .route("/knowledge/{id}", get(get_one).put(update).delete(remove))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateKnowledge>,
) -> Result<Json<Knowledge>, AppError> {
    let pkg = state.pkg_cache.get_for_project(project_id).await?;
    validation::validate_create_knowledge(&req, pkg.as_ref())?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "create").await?;
    let k = state.db.create_knowledge(project_id, &req, user_id).await?;

    // Spawn background task to compute and store the embedding.
    let db = state.db.clone();
    let embedder = state.embedder.clone();
    let kid = k.id;
    let text = format!("{}\n\n{}", req.title, req.content);
    tokio::spawn(async move {
        match embedder.embed(&text).await {
            Ok(Some(vec)) => {
                if let Err(e) = db.update_knowledge_embedding(kid, &vec).await {
                    tracing::warn!(id = %kid, error = %e, "Failed to store knowledge embedding");
                }
            }
            Ok(None) => {} // embedding provider not configured
            Err(e) => tracing::warn!(id = %kid, error = %e, "Embedding request failed"),
        }
    });

    Ok(Json(k))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<KnowledgeFilters>,
) -> Result<Json<PaginatedResponse<Knowledge>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_knowledge(project_id, &filters),
        state.db.count_knowledge(project_id, &filters),
    )
    .await
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Knowledge>, AppError> {
    let k = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_knowledge_by_id(id).await?,
    )
    .await?;
    Ok(Json(k))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateKnowledge>,
) -> Result<Json<Knowledge>, AppError> {
    let existing = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_knowledge_by_id(id).await?,
        "manage",
    )
    .await?;
    let pkg = state.pkg_cache.get_for_project(existing.project_id).await?;
    validation::validate_update_knowledge(&req, pkg.as_ref())?;
    let k = state.db.update_knowledge(id, &req).await?;
    Ok(Json(k))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_knowledge_by_id(id).await?,
        "manage",
    )
    .await?;
    state.db.delete_knowledge(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
