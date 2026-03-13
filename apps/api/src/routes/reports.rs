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

/// Researcher playbook UUID (built-in seed playbook).
const RESEARCHER_PLAYBOOK_ID: &str = "e701aa5c-02e1-47f6-9d9c-fa7e125f578c";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/reports", post(create).get(list))
        .route("/{project_id}/reports/{id}/complete", post(complete))
        .route("/reports/{id}", get(get_one).put(update).delete(remove))
}

async fn create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Json(req): Json<CreateReport>,
) -> Result<Json<Report>, AppError> {
    validation::validate_create_report(&req)?;
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "create").await?;
    let r = state.db.create_report(project_id, &req, user_id).await?;

    state.fire_event(
        project_id,
        "report.created",
        "report",
        r.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"report_id": r.id, "title": r.title, "kind": r.kind}),
    );

    // Auto-spawn a research task using the Researcher playbook.
    let playbook_id: Uuid = RESEARCHER_PLAYBOOK_ID
        .parse()
        .expect("hardcoded researcher playbook UUID");

    let task_context = serde_json::json!({
        "spec": format!(
            "You are researching for report \"{}\" (kind: {}).\n\n\
             ## Research Prompt\n\n{}\n\n\
             ## Instructions\n\n\
             When you have completed your research, post the final result by calling:\n\
             ```\n\
             curl -s -X POST $API_BASE/v1/{}/reports/{}/complete \\\n\
               -H 'Content-Type: application/json' \\\n\
               -H \"$AUTH_HEADER\" -H \"X-Agent-Id: $AGENT_ID\" \\\n\
               -d '{{\"result\": \"<your markdown result>\", \"status\": \"completed\"}}'\n\
             ```\n\
             Replace $API_BASE, $AUTH_HEADER, and $AGENT_ID with the values from your environment.",
            r.title, r.kind, r.prompt, project_id, r.id
        ),
        "report_id": r.id.to_string(),
        "notes": format!("This task was auto-created for report {}. Post results to POST /{}/reports/{}/complete", r.id, project_id, r.id),
    });

    let create_task = CreateTask {
        title: format!("Report: {}", r.title),
        kind: Some("research".to_string()),
        urgent: None,
        context: Some(task_context),
        required_capabilities: None,
        playbook_id: Some(playbook_id),
        decision_id: None,
        goal_id: None,
        file_scope: None,
        parent_id: None,
        plan_id: None,
    };

    match state
        .db
        .create_task(project_id, &create_task, user_id)
        .await
    {
        Ok(task) => {
            // Update report with the spawned task_id and set status to in_progress
            let update_req = UpdateReport {
                title: None,
                status: Some("in_progress".to_string()),
                result: None,
                task_id: Some(task.id),
                metadata: None,
            };
            let r = state.db.update_report(r.id, &update_req).await?;

            // Fire task creation event
            if task.state == "ready" {
                state.fire_event(
                    project_id,
                    "task.transitioned",
                    "task",
                    task.id,
                    agent_id,
                    Some(user_id),
                    serde_json::json!({
                        "task_id": task.id,
                        "title": task.title,
                        "from": "backlog",
                        "to": "ready",
                        "playbook_id": task.playbook_id,
                        "playbook_step": task.playbook_step,
                        "report_id": r.id,
                    }),
                );
            }

            Ok(Json(r))
        }
        Err(e) => {
            // Task creation failed, but report was already created — return it as-is.
            tracing::warn!(
                report_id = %r.id,
                error = %e,
                "Failed to auto-spawn research task for report"
            );
            Ok(Json(r))
        }
    }
}

async fn list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(project_id): Path<Uuid>,
    Query(filters): Query<ReportFilters>,
) -> Result<Json<PaginatedResponse<Report>>, AppError> {
    require_membership(state.db.as_ref(), agent_id, user_id, project_id).await?;
    super::paginate(
        filters.limit,
        filters.offset,
        state.db.list_reports(project_id, &filters),
        state.db.count_reports(project_id, &filters),
    )
    .await
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<Json<Report>, AppError> {
    let r = ensure_member(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_report_by_id(id).await?,
    )
    .await?;
    Ok(Json(r))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReport>,
) -> Result<Json<Report>, AppError> {
    let existing = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_report_by_id(id).await?,
        "execute",
    )
    .await?;
    validation::validate_update_report(&req)?;
    let r = state.db.update_report(id, &req).await?;

    state.fire_event(
        existing.project_id,
        "report.updated",
        "report",
        r.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"report_id": r.id, "title": r.title, "status": r.status}),
    );

    Ok(Json(r))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let existing = ensure_authority_on(
        state.db.as_ref(),
        agent_id,
        user_id,
        state.db.get_report_by_id(id).await?,
        "manage",
    )
    .await?;
    state.db.delete_report(id).await?;

    state.fire_event(
        existing.project_id,
        "report.deleted",
        "report",
        id,
        agent_id,
        Some(user_id),
        serde_json::json!({"report_id": id, "title": existing.title}),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// `POST /{project_id}/reports/{id}/complete`
///
/// Allows a research agent to post the final result for a report.
async fn complete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CompleteReport>,
) -> Result<Json<Report>, AppError> {
    require_authority(state.db.as_ref(), agent_id, user_id, project_id, "execute").await?;

    let existing = state.db.get_report_by_id(id).await?;
    if existing.project_id != project_id {
        return Err(AppError::NotFound(
            "Report not found in this project".into(),
        ));
    }

    let status = req.status.as_deref().unwrap_or("completed").to_string();

    let update_req = UpdateReport {
        title: None,
        status: Some(status),
        result: Some(req.result),
        task_id: None,
        metadata: None,
    };

    validation::validate_update_report(&update_req)?;
    let r = state.db.update_report(id, &update_req).await?;

    state.fire_event(
        project_id,
        "report.completed",
        "report",
        r.id,
        agent_id,
        Some(user_id),
        serde_json::json!({"report_id": r.id, "title": r.title, "status": r.status}),
    );

    Ok(Json(r))
}
