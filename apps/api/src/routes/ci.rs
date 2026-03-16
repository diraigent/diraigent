//! REST API endpoints for querying CI pipeline data and registering Forgejo integrations.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::{OptionalAgentId, require_authority, require_membership};
use crate::error::AppError;
use crate::models::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/ci/runs", get(list_runs))
        .route("/{project_id}/ci/runs/{run_id}", get(get_run))
        .route("/{project_id}/ci/runs/{run_id}/jobs/{job_id}", get(get_job))
        .route(
            "/{project_id}/integrations/forgejo",
            post(register_forgejo_integration),
        )
}

/// GET /{project_id}/ci/runs
///
/// Paginated list of CI runs for a project, ordered by started_at desc.
/// Supports optional query filters: `branch`, `status`, `workflow_name`.
/// Pagination via `page` (1-based, default 1) and `per_page` (default 50, max 100).
async fn list_runs(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<CiRunFilters>,
) -> Result<Json<PaginatedResponse<CiRun>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let per_page = filters.per_page.unwrap_or(50).clamp(1, 100);
    let page = filters.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    super::paginate(
        Some(per_page),
        Some(offset),
        state.db.list_ci_runs(
            project_id,
            filters.branch.as_deref(),
            filters.status.as_deref(),
            filters.workflow_name.as_deref(),
            filters.provider.as_deref(),
            per_page,
            offset,
        ),
        state.db.count_ci_runs(
            project_id,
            filters.branch.as_deref(),
            filters.status.as_deref(),
            filters.workflow_name.as_deref(),
            filters.provider.as_deref(),
        ),
    )
    .await
}

/// GET /{project_id}/ci/runs/{run_id}
///
/// Returns a single CI run with its jobs embedded in the response.
async fn get_run(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CiRunWithJobs>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    let run = state.db.get_ci_run(run_id).await?;
    if run.project_id != project_id {
        return Err(AppError::NotFound("CI run not found".into()));
    }

    let jobs = state.db.list_ci_jobs_for_run(run.id).await?;

    Ok(Json(CiRunWithJobs { run, jobs }))
}

/// GET /{project_id}/ci/runs/{run_id}/jobs/{job_id}
///
/// Returns a single CI job with its steps embedded in the response.
async fn get_job(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, run_id, job_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<CiJobWithSteps>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;

    // Verify the run belongs to the project
    let run = state.db.get_ci_run(run_id).await?;
    if run.project_id != project_id {
        return Err(AppError::NotFound("CI run not found".into()));
    }

    // Verify the job belongs to the run
    let job = state.db.get_ci_job(job_id).await?;
    if job.ci_run_id != run_id {
        return Err(AppError::NotFound("CI job not found".into()));
    }

    let steps = state.db.list_ci_steps_for_job(job.id).await?;

    Ok(Json(CiJobWithSteps { job, steps }))
}

/// POST /{project_id}/integrations/forgejo
///
/// Register a new Forgejo integration for a project. Generates a webhook
/// URL and secret for configuring the Forgejo webhook.
async fn register_forgejo_integration(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateForgejoIntegration>,
) -> Result<Json<ForgejoIntegrationResponse>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "manage").await?;

    // Generate a webhook secret (256-bit entropy via two UUIDs)
    let webhook_secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());

    let integration = state
        .db
        .create_forgejo_integration(
            project_id,
            &req.base_url,
            req.token.as_deref(),
            &webhook_secret,
        )
        .await?;

    // Construct the webhook URL
    let api_base = std::env::var("PUBLIC_URL")
        .or_else(|_| std::env::var("API_BASE_URL"))
        .unwrap_or_else(|_| "https://api.diraigent.com".to_string());
    let webhook_url = format!(
        "{}/v1/webhooks/forgejo/{}",
        api_base.trim_end_matches('/'),
        integration.id
    );

    Ok(Json(ForgejoIntegrationResponse {
        id: integration.id,
        project_id: integration.project_id,
        base_url: integration.base_url,
        webhook_url,
        webhook_secret,
        enabled: integration.enabled,
        created_at: integration.created_at,
        updated_at: integration.updated_at,
    }))
}
