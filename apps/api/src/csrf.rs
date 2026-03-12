//! CSRF protection middleware.
//!
//! This API uses Bearer token authentication (JWT in the `Authorization` header),
//! which is not inherently vulnerable to CSRF attacks because browsers do not
//! automatically include `Authorization` headers in cross-site requests.
//!
//! However, as a defence-in-depth measure this middleware validates the `Origin`
//! (or `Referer`) header on all state-mutating requests (POST, PUT, PATCH, DELETE)
//! when an explicit allow-list is configured via `CORS_ORIGINS`.
//!
//! Rules:
//! - `CORS_ORIGINS` not set → dev/open mode, skip enforcement.
//! - Request has no `Origin` or `Referer` → direct/non-browser client (e.g. agent
//!   CLI, curl) → allow.
//! - Request `Origin` is in the `CORS_ORIGINS` list → allow.
//! - Any other origin → 403 Forbidden.

use axum::{
    body::Body,
    http::{Method, Request, Response, StatusCode},
    middleware::Next,
};
use std::env;

/// Extract `scheme://host` from a Referer URL string without an external crate.
fn origin_from_referer(referer: &str) -> Option<String> {
    // Expected format: "scheme://host[/path...]"
    let sep = referer.find("://")?;
    let scheme = &referer[..sep];
    let after = &referer[sep + 3..];
    let host_len = after.find('/').unwrap_or(after.len());
    let host = &after[..host_len];
    if scheme.is_empty() || host.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{host}"))
}

/// Axum middleware that enforces Origin-based CSRF checks on mutating requests.
pub async fn csrf_check(request: Request<Body>, next: Next) -> Response<Body> {
    // Only mutating methods are relevant for CSRF.
    let is_mutating = matches!(
        request.method(),
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    );
    if !is_mutating {
        return next.run(request).await;
    }

    // When CORS_ORIGINS is not configured, we are in unrestricted/dev mode.
    let cors_origins = match env::var("CORS_ORIGINS") {
        Ok(o) if !o.trim().is_empty() => o,
        _ => return next.run(request).await,
    };

    // Extract the request origin from the `Origin` header, falling back to
    // deriving it from the `Referer` header.
    let request_origin: Option<String> = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            request
                .headers()
                .get("referer")
                .and_then(|v| v.to_str().ok())
                .and_then(origin_from_referer)
        });

    // Non-browser clients (agents, CLI tools, curl) typically send no Origin.
    // Allow them through — they cannot be the subject of a CSRF attack.
    let Some(origin) = request_origin else {
        return next.run(request).await;
    };

    let allowed = cors_origins.split(',').map(str::trim).any(|a| a == origin);

    if allowed {
        return next.run(request).await;
    }

    tracing::warn!(
        method = %request.method(),
        path = %request.uri().path(),
        origin = %origin,
        "CSRF check rejected request: origin not in CORS_ORIGINS allow-list"
    );

    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"error":"forbidden: origin not allowed"}"#))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_origin_from_referer() {
        assert_eq!(
            origin_from_referer("https://app.example.com/some/path"),
            Some("https://app.example.com".to_string())
        );
        assert_eq!(
            origin_from_referer("http://localhost:3000/"),
            Some("http://localhost:3000".to_string())
        );
        assert_eq!(
            origin_from_referer("https://example.com"),
            Some("https://example.com".to_string())
        );
        assert_eq!(origin_from_referer("not-a-url"), None);
        assert_eq!(origin_from_referer("://missing-scheme"), None);
    }
}
