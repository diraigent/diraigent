use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Validation(String),
    Conflict(String),
    Internal(String),
    Unauthorized(String),
    Forbidden(String),
    UnprocessableEntity(String),
    ServiceUnavailable(String),
    BadGateway(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "{msg}"),
            AppError::Validation(msg) => write!(f, "{msg}"),
            AppError::Conflict(msg) => write!(f, "{msg}"),
            AppError::Internal(msg) => write!(f, "{msg}"),
            AppError::Unauthorized(msg) => write!(f, "{msg}"),
            AppError::Forbidden(msg) => write!(f, "{msg}"),
            AppError::UnprocessableEntity(msg) => write!(f, "{msg}"),
            AppError::ServiceUnavailable(msg) => write!(f, "{msg}"),
            AppError::BadGateway(msg) => write!(f, "{msg}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "FORBIDDEN", msg),
            AppError::UnprocessableEntity(msg) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "INVALID_TRANSITION", msg)
            }
            AppError::ServiceUnavailable(msg) => {
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE", msg)
            }
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, "BAD_GATEWAY", msg),
        };

        let body = axum::Json(json!({
            "error": message,
            "errorCode": code,
        }));

        (status, body).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = e {
            // PostgreSQL error codes: https://www.postgresql.org/docs/current/errcodes-appendix.html
            if let Some(code) = db_err.code() {
                return match code.as_ref() {
                    // 23505 = unique_violation
                    "23505" => {
                        let detail = db_err
                            .constraint()
                            .map(|c| format!("Duplicate value violates constraint: {}", c))
                            .unwrap_or_else(|| "A record with this value already exists".into());
                        AppError::Conflict(detail)
                    }
                    // 23503 = foreign_key_violation
                    "23503" => {
                        let detail = db_err
                            .constraint()
                            .map(|c| format!("Referenced entity not found: {}", c))
                            .unwrap_or_else(|| "Referenced entity does not exist".into());
                        AppError::Validation(detail)
                    }
                    // 23514 = check_violation
                    "23514" => {
                        let detail = db_err
                            .constraint()
                            .map(|c| format!("Value violates constraint: {}", c))
                            .unwrap_or_else(|| "Invalid value for field".into());
                        AppError::Validation(detail)
                    }
                    _ => {
                        tracing::error!(error = %e, code = %code, "Database error");
                        AppError::Internal("Internal server error".into())
                    }
                };
            }
        }
        tracing::error!(error = %e, "Database error");
        AppError::Internal("Internal server error".into())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "Internal error");
        AppError::Internal("Internal server error".into())
    }
}
