use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;

pub fn routes() -> Router<AppState> {
    Router::new().route("/account", get(get_account).delete(delete_account))
}

#[derive(Serialize)]
struct AccountResponse {
    user_id: Uuid,
}

/// `GET /v1/account`
///
/// Returns the authenticated user's internal ID.
async fn get_account(AuthUser(user_id): AuthUser) -> Json<AccountResponse> {
    Json(AccountResponse { user_id })
}

#[derive(Serialize)]
struct DeletedResponse {
    deleted: bool,
}

/// `DELETE /v1/account`
///
/// Permanently deletes the authenticated user's account.
/// Nullifies all references (audit logs, comments, updates, etc.) and removes
/// the auth_user row (cascading tenant_member and wrapped_key).
async fn delete_account(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> Result<(StatusCode, Json<DeletedResponse>), AppError> {
    state.db.delete_user_account(user_id).await?;
    Ok((StatusCode::OK, Json(DeletedResponse { deleted: true })))
}
