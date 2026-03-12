use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::AppState;
use crate::auth::AuthUser;

pub fn routes() -> Router<AppState> {
    Router::new().route("/settings", get(get_settings))
}

#[derive(Debug, Serialize)]
pub struct Settings {
    /// Absolute base directory for all project paths.
    /// When set, project `repo_path` values are relative to this directory.
    /// Corresponds to the `PROJECTS_PATH` environment variable.
    pub projects_path: Option<String>,
    /// Git repository root directory.
    /// When set, git API endpoints are enabled.
    /// Corresponds to the `REPO_ROOT` environment variable.
    pub repo_root: Option<String>,
}

async fn get_settings(State(state): State<AppState>, AuthUser(_): AuthUser) -> Json<Settings> {
    Json(Settings {
        projects_path: state
            .projects_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        repo_root: state
            .repo_root
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
    })
}
