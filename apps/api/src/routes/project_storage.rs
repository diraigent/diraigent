use crate::{
    AppState,
    auth::AuthUser,
    authz::{OptionalAgentId, require_authority, require_membership},
    error::AppError,
    project_content,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use diraigent_types::project_content::ContentRequest;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project_id}/storage", get(status).put(assign))
        .route("/{project_id}/storage/migrate", post(migrate))
        .route("/{project_id}/chat/history/clear", post(clear_history))
        .route(
            "/{project_id}/chat/history",
            get(history).delete(clear_history),
        )
}
async fn status(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    OptionalAgentId(a): OptionalAgentId,
    Path(p): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_membership(s.db.as_ref(), a, u, p).await?;
    Ok(Json(
        json!({"agent_id":project_content::owner(&s,p).await?,"content_protocol":2,"heavy_content":sqlx::query_scalar::<_,bool>("SELECT heavy_content FROM diraigent.project_content_owner WHERE project_id=$1").bind(p).fetch_optional(&s.pool).await?.unwrap_or(false)}),
    ))
}
#[derive(Deserialize)]
struct Assignment {
    agent_id: Uuid,
    #[serde(default)]
    heavy_content: bool,
}
async fn assign(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    OptionalAgentId(a): OptionalAgentId,
    Path(p): Path<Uuid>,
    Json(req): Json<Assignment>,
) -> Result<Json<Value>, AppError> {
    require_authority(s.db.as_ref(), a, u, p, "manage").await?;
    let project = s.db.get_project_by_id(p).await?;
    let tenant = s.db.get_tenant_by_id(project.tenant_id).await?;
    if tenant.encryption_mode != "none" {
        return Err(AppError::Validation("Orchestra content storage requires an unencrypted workspace; local encryption support is not yet available".into()));
    }
    if !s
        .db
        .list_tenant_agent_ids(project.tenant_id)
        .await?
        .contains(&req.agent_id)
    {
        return Err(AppError::Validation(
            "Storage owner must belong to this workspace".into(),
        ));
    }
    if project_content::owner(&s, p)
        .await?
        .is_some_and(|old| old != req.agent_id)
    {
        return Err(AppError::Conflict(
            "Storage owner transfer requires copying and verifying its data first".into(),
        ));
    }
    let probe = project_content::request(&s, p, req.agent_id, ContentRequest::Probe).await?;
    if probe["protocol"].as_u64().unwrap_or(0) < if req.heavy_content { 2 } else { 1 } {
        return Err(AppError::Validation(
            "Unsupported Orchestra content protocol".into(),
        ));
    }
    let store_id: Uuid = serde_json::from_value(probe["store_id"].clone())
        .map_err(|_| AppError::Validation("Missing Orchestra store identity".into()))?;
    let updated = sqlx::query(
        "INSERT INTO diraigent.project_content_owner (project_id,agent_id,store_id,heavy_content) VALUES ($1,$2,$3,$4) ON CONFLICT (project_id) DO UPDATE SET heavy_content=diraigent.project_content_owner.heavy_content OR EXCLUDED.heavy_content WHERE diraigent.project_content_owner.agent_id=EXCLUDED.agent_id AND diraigent.project_content_owner.store_id=EXCLUDED.store_id",
    )
    .bind(p)
    .bind(req.agent_id)
    .bind(store_id)
    .bind(req.heavy_content)
    .execute(&s.pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "Storage owner changed concurrently".into(),
        ));
    }
    let heavy: bool = sqlx::query_scalar(
        "SELECT heavy_content FROM diraigent.project_content_owner WHERE project_id=$1",
    )
    .bind(p)
    .fetch_one(&s.pool)
    .await?;
    Ok(Json(
        json!({"agent_id":req.agent_id,"content_protocol":2,"heavy_content":heavy}),
    ))
}
async fn migrate(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    OptionalAgentId(a): OptionalAgentId,
    Path(p): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_authority(s.db.as_ref(), a, u, p, "manage").await?;
    if project_content::owner(&s, p).await?.is_none() {
        return Err(AppError::Validation("Assign a storage owner first".into()));
    }
    Ok(Json(project_content::migrate(&s, p).await?))
}
async fn history(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    OptionalAgentId(a): OptionalAgentId,
    Path(p): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_membership(s.db.as_ref(), a, u, p).await?;
    match project_content::owner(&s, p).await? {
        Some(owner) => Ok(Json(
            project_content::request(&s, p, owner, ContentRequest::History { user_id: u }).await?,
        )),
        None => Ok(Json(
            json!({"enabled":false,"revision":0,"messages":[],"busy":false}),
        )),
    }
}
#[derive(Deserialize)]
struct Clear {
    revision: i64,
}
async fn clear_history(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    OptionalAgentId(a): OptionalAgentId,
    Path(p): Path<Uuid>,
    Json(req): Json<Clear>,
) -> Result<Json<Value>, AppError> {
    require_membership(s.db.as_ref(), a, u, p).await?;
    let owner = project_content::owner(&s, p)
        .await?
        .ok_or_else(|| AppError::Validation("No storage owner configured".into()))?;
    Ok(Json(
        project_content::request(
            &s,
            p,
            owner,
            ContentRequest::ClearHistory {
                user_id: u,
                revision: req.revision,
            },
        )
        .await?,
    ))
}
