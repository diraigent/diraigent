use crate::{AppState, auth::AuthUser, error::AppError, project_access};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/{project}/people", get(list).post(grant))
        .route("/{project}/people/me", get(my_access))
        .route("/{project}/people/{user}", axum::routing::delete(remove))
}
#[derive(Serialize, sqlx::FromRow)]
struct Person {
    user_id: Uuid,
    role: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    user_id: Uuid,
    role: String,
}
async fn require_manager(state: &AppState, user: Uuid, project: Uuid) -> Result<(), AppError> {
    if project_access::role(&state.pool, user, project)
        .await?
        .as_deref()
        != Some("manager")
    {
        return Err(AppError::Forbidden(
            "Project manager access required".into(),
        ));
    }
    Ok(())
}
async fn my_access(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    Path(p): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let role = project_access::role(&s.pool, u, p)
        .await?
        .ok_or_else(|| AppError::Forbidden("No project access".into()))?;
    Ok(Json(
        serde_json::json!({"role":role,"read_only":role=="viewer"}),
    ))
}
async fn list(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    Path(p): Path<Uuid>,
) -> Result<Json<Vec<Person>>, AppError> {
    require_manager(&s, u, p).await?;
    Ok(Json(sqlx::query_as("SELECT user_id,role FROM diraigent.project_user_access WHERE project_id=$1 ORDER BY user_id").bind(p).fetch_all(&s.pool).await?))
}
async fn grant(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    Path(p): Path<Uuid>,
    Json(g): Json<Grant>,
) -> Result<Json<Person>, AppError> {
    require_manager(&s, u, p).await?;
    if !matches!(g.role.as_str(), "viewer" | "editor" | "manager") {
        return Err(AppError::Validation("Unknown project role".into()));
    }
    let role=sqlx::query_scalar::<_,String>("SELECT m.role FROM diraigent.tenant_member m JOIN diraigent.project p ON p.tenant_id=m.tenant_id WHERE p.id=$1 AND m.user_id=$2").bind(p).bind(g.user_id).fetch_optional(&s.pool).await?
        .ok_or_else(||AppError::Validation("Add this user to the workspace first".into()))?;
    if matches!(role.as_str(), "owner" | "admin") {
        return Err(AppError::Validation(
            "Workspace administrators already manage every project".into(),
        ));
    }
    if role == "viewer" && g.role != "viewer" {
        return Err(AppError::Validation(
            "A read-only account cannot receive write access".into(),
        ));
    }
    Ok(Json(sqlx::query_as("INSERT INTO diraigent.project_user_access(project_id,user_id,role) VALUES($1,$2,$3) ON CONFLICT(project_id,user_id) DO UPDATE SET role=EXCLUDED.role RETURNING user_id,role").bind(p).bind(g.user_id).bind(&g.role).fetch_one(&s.pool).await?))
}
async fn remove(
    State(s): State<AppState>,
    AuthUser(u): AuthUser,
    Path((p, target)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    require_manager(&s, u, p).await?;
    sqlx::query("DELETE FROM diraigent.project_user_access WHERE project_id=$1 AND user_id=$2")
        .bind(p)
        .bind(target)
        .execute(&s.pool)
        .await?;
    Ok(())
}
