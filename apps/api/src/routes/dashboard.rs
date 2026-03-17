use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::OptionalAgentId;
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new().route("/dashboard/summary", get(get_dashboard_summary))
}

/// `GET /dashboard/summary?days=30`
///
/// Returns aggregated dashboard data across all projects the user has access to
/// in a single request, replacing the N*4 per-project polling pattern.
async fn get_dashboard_summary(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(_agent_id): OptionalAgentId,
    Query(query): Query<DashboardQuery>,
) -> Result<Json<DashboardSummary>, AppError> {
    let days = query.days.unwrap_or(30).clamp(1, 365);

    // Get all projects for this user's tenant
    let tenant = state
        .db
        .get_tenant_for_user(user_id)
        .await?
        .ok_or_else(|| AppError::Forbidden("User has no tenant".into()))?;

    let projects = state
        .db
        .list_projects_for_tenant(
            tenant.id,
            &Pagination {
                limit: Some(100),
                offset: Some(0),
            },
        )
        .await?;

    // Gather per-project data concurrently
    let mut project_summaries = Vec::with_capacity(projects.len());
    let mut all_tokens_per_day = Vec::new();

    let work_filters = WorkFilters::default();
    for project in projects {
        let (metrics, works) = tokio::try_join!(
            state.db.get_project_metrics(project.id, days),
            state.db.list_works(project.id, &work_filters),
        )?;

        let active_work: Vec<Work> = works
            .into_iter()
            .filter(|w| w.status != "achieved" && w.status != "abandoned")
            .collect();

        all_tokens_per_day.extend(metrics.tokens_per_day);

        project_summaries.push(DashboardProjectSummary {
            project,
            task_summary: metrics.task_summary,
            active_work,
            cost_summary: metrics.cost_summary,
        });
    }

    Ok(Json(DashboardSummary {
        projects: project_summaries,
        tokens_per_day: all_tokens_per_day,
    }))
}
