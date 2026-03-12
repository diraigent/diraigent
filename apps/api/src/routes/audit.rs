use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_membership};
use crate::error::AppError;
use crate::models::{AuditEntry, AuditFilters, PaginatedResponse};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/audit", get(list_audit))
        .route("/audit/{entity_type}/{entity_id}", get(entity_history))
}

async fn list_audit(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<AuditFilters>,
) -> Result<Json<PaginatedResponse<AuditEntry>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_audit_log(project_id, &filters),
        state.db.count_audit_log(project_id, &filters),
    )
    .await
}

async fn entity_history(
    State(state): State<AppState>,
    AuthUser(_user_id): AuthUser,
    Path((entity_type, entity_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<AuditEntry>>, AppError> {
    let entries = state
        .db
        .get_entity_history(&entity_type, entity_id, 100)
        .await?;
    Ok(Json(entries))
}
