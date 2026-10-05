//! Anonymous publication routes. Never use authenticated extractors or CryptoDb:
//! foreign detail IDs must not cause key lookup/decryption before scoping.
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    db::{DiraigentDb, PostgresDb},
    error::AppError,
    models::{
        DecisionFilters, KnowledgeFilters, PaginatedResponse, Project, TaskFilters, WorkFilters,
    },
    spectator::*,
};

pub fn routes() -> Router<AppState> {
    Router::new().nest("/spectator", publication_routes())
}

fn publication_routes() -> Router<AppState> {
    Router::new()
        .route("/projects/{project_id}", get(project))
        .route("/projects/{project_id}/tasks", get(tasks))
        .route("/projects/{project_id}/tasks/{id}", get(task))
        .route("/projects/{project_id}/work", get(work_list))
        .route("/projects/{project_id}/work/{id}", get(work))
        .route("/projects/{project_id}/knowledge", get(knowledge_list))
        .route("/projects/{project_id}/knowledge/{id}", get(knowledge))
        .route("/projects/{project_id}/decisions", get(decisions))
        .route("/projects/{project_id}/decisions/{id}", get(decision))
        .fallback(|| async { not_found() })
        .layer(middleware::from_fn(read_only_no_store))
}

async fn read_only_no_store(req: Request, next: Next) -> Response {
    // Axum GET handlers otherwise also run on HEAD. This is a strict GET allowlist.
    let mut response = if req.method() != Method::GET {
        (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "GET")]).into_response()
    } else {
        next.run(req).await
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

fn not_found() -> AppError {
    AppError::NotFound("Spectator project not found".into())
}

fn hide_missing(error: AppError) -> AppError {
    match error {
        AppError::NotFound(_) => not_found(),
        other => other,
    }
}

async fn access(db: &dyn DiraigentDb, id: Uuid) -> Result<Project, AppError> {
    let project = db.get_project_by_id(id).await.map_err(hide_missing)?;
    let tenant = match db.get_tenant_by_id(project.tenant_id).await {
        Ok(tenant) => Some(tenant),
        Err(AppError::NotFound(_)) => None,
        Err(error) => return Err(error),
    };
    ensure_access(&project, tenant.as_ref())?;
    Ok(project)
}

fn scoped(actual: Uuid, requested: Uuid) -> Result<(), AppError> {
    if actual != requested {
        return Err(not_found());
    }
    Ok(())
}

#[derive(Default, Deserialize)]
struct PageQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

impl PageQuery {
    fn bounds(self) -> Result<(i64, i64), AppError> {
        let offset = self.offset.unwrap_or(0);
        if offset < 0 {
            return Err(AppError::Validation("offset must be nonnegative".into()));
        }
        Ok((
            self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
            offset,
        ))
    }
}

fn page<T: Serialize>(
    data: Vec<T>,
    total: i64,
    limit: i64,
    offset: i64,
) -> Json<PaginatedResponse<T>> {
    Json(PaginatedResponse {
        data,
        total,
        limit,
        offset,
        has_more: offset.saturating_add(limit) < total,
    })
}

async fn project(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PublicProject>, AppError> {
    let db = PostgresDb(state.pool);
    Ok(Json(PublicProject::from(&access(&db, id).await?)))
}

async fn tasks(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PaginatedResponse<PublicTask>>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, id).await?;
    let (limit, offset) = query.bounds()?;
    let filters = TaskFilters {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    let (rows, total) =
        tokio::try_join!(db.list_tasks(id, &filters), db.count_tasks(id, &filters))?;
    for row in &rows {
        scoped(row.project_id, id)?;
    }
    Ok(page(
        rows.iter().map(PublicTask::from).collect(),
        total,
        limit,
        offset,
    ))
}

async fn task(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicTaskDetail>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, project_id).await?;
    let row = db.get_task_by_id(id).await.map_err(hide_missing)?;
    scoped(row.project_id, project_id)?;
    Ok(Json(PublicTaskDetail::from(&row)))
}

async fn work_list(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PaginatedResponse<PublicWork>>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, id).await?;
    let (limit, offset) = query.bounds()?;
    let filters = WorkFilters {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    let (rows, counts) = tokio::try_join!(db.list_works(id, &filters), db.work_status_counts(id))?;
    for row in &rows {
        scoped(row.project_id, id)?;
    }
    Ok(page(
        rows.iter().map(PublicWork::from).collect(),
        counts.iter().map(|(_, count)| count).sum(),
        limit,
        offset,
    ))
}

async fn work(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicWork>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, project_id).await?;
    let row = db.get_work_by_id(id).await.map_err(hide_missing)?;
    scoped(row.project_id, project_id)?;
    Ok(Json(PublicWork::from(&row)))
}

async fn knowledge_list(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PaginatedResponse<PublicKnowledge>>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, id).await?;
    let (limit, offset) = query.bounds()?;
    let filters = KnowledgeFilters {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    let (rows, total) = tokio::try_join!(
        db.list_knowledge(id, &filters),
        db.count_knowledge(id, &filters)
    )?;
    for row in &rows {
        scoped(row.project_id, id)?;
    }
    Ok(page(
        rows.iter().map(PublicKnowledge::from).collect(),
        total,
        limit,
        offset,
    ))
}

async fn knowledge(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicKnowledge>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, project_id).await?;
    let row = db.get_knowledge_by_id(id).await.map_err(hide_missing)?;
    scoped(row.project_id, project_id)?;
    Ok(Json(PublicKnowledge::from(&row)))
}

async fn decisions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PaginatedResponse<PublicDecision>>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, id).await?;
    let (limit, offset) = query.bounds()?;
    let filters = DecisionFilters {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    let (rows, total) = tokio::try_join!(
        db.list_decisions(id, &filters),
        db.count_decisions(id, &filters)
    )?;
    for row in &rows {
        scoped(row.project_id, id)?;
    }
    Ok(page(
        rows.iter().map(PublicDecision::from).collect(),
        total,
        limit,
        offset,
    ))
}

async fn decision(
    State(state): State<AppState>,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicDecision>, AppError> {
    let db = PostgresDb(state.pool);
    access(&db, project_id).await?;
    let row = db.get_decision_by_id(id).await.map_err(hide_missing)?;
    scoped(row.project_id, project_id)?;
    Ok(Json(PublicDecision::from(&row)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectator_page_bounds_are_bounded_and_overflow_safe() {
        assert_eq!(PageQuery::default().bounds().unwrap(), (20, 0));
        for (input, expected) in [(i64::MIN, 1), (0, 1), (1, 1), (100, 100), (i64::MAX, 100)] {
            assert_eq!(
                PageQuery {
                    limit: Some(input),
                    offset: Some(i64::MAX)
                }
                .bounds()
                .unwrap(),
                (expected, i64::MAX)
            );
        }
        assert!(
            PageQuery {
                limit: None,
                offset: Some(-1)
            }
            .bounds()
            .is_err()
        );
        let Json(result) = page(Vec::<PublicTask>::new(), 10, 100, i64::MAX);
        assert!(!result.has_more);
    }
}
