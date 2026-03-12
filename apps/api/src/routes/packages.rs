use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::authz::OptionalAgentId;
use crate::error::AppError;
use crate::models::*;

use crate::validation;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/packages", post(create).get(list))
        .route("/packages/{id}", get(get_one).put(update).delete(remove))
}

/// Packages are global config — only human users can mutate them.
fn reject_agent(agent_id: Option<Uuid>) -> Result<(), AppError> {
    if agent_id.is_some() {
        return Err(AppError::Forbidden("Agents cannot modify packages".into()));
    }
    Ok(())
}

async fn create(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Json(req): Json<CreatePackage>,
) -> Result<(StatusCode, Json<Package>), AppError> {
    reject_agent(agent_id)?;
    validation::validate_create_package(&req)?;
    let pkg = state.db.create_package(&req).await?;
    Ok((StatusCode::CREATED, Json(pkg)))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
) -> Result<Json<Vec<Package>>, AppError> {
    let pkgs = state.db.list_packages().await?;
    Ok(Json(pkgs))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Package>, AppError> {
    let pkg = if let Ok(uuid) = id.parse::<Uuid>() {
        state.db.get_package_by_id(uuid).await?
    } else {
        state.db.get_package_by_slug(&id).await?
    };
    Ok(Json(pkg))
}

async fn update(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePackage>,
) -> Result<Json<Package>, AppError> {
    reject_agent(agent_id)?;
    validation::validate_update_package(&req)?;
    let pkg = state.db.update_package(id, &req).await?;
    Ok(Json(pkg))
}

async fn remove(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    OptionalAgentId(agent_id): OptionalAgentId,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    reject_agent(agent_id)?;
    state.db.delete_package(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
