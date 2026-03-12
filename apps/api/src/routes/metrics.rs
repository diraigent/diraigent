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
    Router::new().route("/{project_id}/metrics", get(get_metrics))
}

async fn get_metrics(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(query): Query<MetricsQuery>,
) -> Result<Json<ProjectMetrics>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    let days = query.days.unwrap_or(30).clamp(1, 365);
    let metrics = state.db.get_project_metrics(project_id, days).await?;
    Ok(Json(metrics))
}
