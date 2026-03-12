use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new().route("/{project_id}/search", get(search))
}

async fn search(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let q = params.q.trim();
    if q.is_empty() {
        return Err(AppError::Validation(
            "query parameter 'q' must not be empty".into(),
        ));
    }

    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);

    let entity_types_parsed: Option<Vec<&str>> = params
        .entity_types
        .as_deref()
        .map(|s| s.split(',').map(|t| t.trim()).collect());

    let (results, total) = state
        .db
        .search(project_id, q, entity_types_parsed.as_deref(), limit, offset)
        .await?;

    Ok(Json(SearchResponse {
        results,
        total,
        query: q.to_string(),
    }))
}
