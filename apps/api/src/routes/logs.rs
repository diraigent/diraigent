use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::tenant::TenantContext;

/// Shared HTTP client with a timeout for Loki requests.
static LOKI_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(3))
        .build()
        .expect("build reqwest client")
});

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    /// LogQL query, e.g. `{app="news-enrichment"}`
    pub query: String,
    /// Start time (RFC3339 or Unix nanoseconds). Defaults to 1h ago.
    pub start: Option<String>,
    /// End time (RFC3339 or Unix nanoseconds). Defaults to now.
    pub end: Option<String>,
    /// Max entries to return. Defaults to 100.
    pub limit: Option<u32>,
    /// Direction: `forward` or `backward` (default).
    pub direction: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LokiResponse {
    status: String,
    data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub line: String,
    pub labels: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub entries: Vec<LogEntry>,
    pub total: usize,
}

fn loki_base_url(state: &AppState) -> String {
    state
        .loki_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3100".to_string())
}

async fn loki_get(base_url: &str, path: &str) -> Result<serde_json::Value, AppError> {
    let url = format!("{base_url}{path}");
    let resp = LOKI_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Loki request failed: {e}")))?;
    let val: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Loki parse failed: {e}")))?;
    Ok(val)
}

/// Query Loki label names.
async fn labels(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    _tenant: TenantContext,
) -> Result<Json<serde_json::Value>, AppError> {
    let val = loki_get(&loki_base_url(&state), "/loki/api/v1/labels").await?;
    Ok(Json(val))
}

/// Query Loki label values for a given label.
async fn label_values(
    State(state): State<AppState>,
    AuthUser(_): AuthUser,
    _tenant: TenantContext,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let val = loki_get(
        &loki_base_url(&state),
        &format!("/loki/api/v1/label/{name}/values"),
    )
    .await?;
    Ok(Json(val))
}

/// Query Loki for log entries.
async fn query_logs(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    tenant: TenantContext,
    Query(params): Query<LogQuery>,
) -> Result<Json<LogsResponse>, AppError> {
    tracing::info!(
        user_id = %user_id,
        tenant_id = %tenant.tenant_id,
        query = %params.query,
        "Log query"
    );
    let loki_url = loki_base_url(&state);
    let limit = params.limit.unwrap_or(100).min(5000);
    let direction = params.direction.as_deref().unwrap_or("backward");

    let mut url = format!(
        "{loki_url}/loki/api/v1/query_range?query={}&limit={limit}&direction={direction}",
        urlencoding::encode(&params.query),
    );
    if let Some(ref start) = params.start {
        url.push_str(&format!("&start={}", urlencoding::encode(start)));
    }
    if let Some(ref end) = params.end {
        url.push_str(&format!("&end={}", urlencoding::encode(end)));
    }

    let resp = LOKI_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Loki request failed: {e}")))?;

    let status = resp.status();
    let body: String = resp
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("Loki read body failed: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Internal(format!(
            "Loki returned {status}: {body}"
        )));
    }

    let loki: LokiResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Internal(format!("Loki response parse failed: {e}")))?;

    let mut entries = parse_streams(&loki.data);

    // Loki returns entries sorted within each stream, but when multiple
    // streams are returned (e.g. query {app=~".+"}), the streams are
    // concatenated without global ordering. Sort entries by timestamp
    // across all streams so the web UI shows them in the correct order.
    entries.sort_by(|a, b| {
        let ta = a.timestamp.parse::<u128>().unwrap_or(0);
        let tb = b.timestamp.parse::<u128>().unwrap_or(0);
        if direction == "forward" {
            ta.cmp(&tb)
        } else {
            tb.cmp(&ta)
        }
    });

    let total = entries.len();

    Ok(Json(LogsResponse { entries, total }))
}

fn parse_streams(data: &serde_json::Value) -> Vec<LogEntry> {
    let mut entries = Vec::new();

    let Some(results) = data.get("result").and_then(|r| r.as_array()) else {
        return entries;
    };

    for stream in results {
        let labels = stream
            .get("stream")
            .cloned()
            .unwrap_or(serde_json::Value::Object(Default::default()));

        if let Some(values) = stream.get("values").and_then(|v| v.as_array()) {
            for pair in values {
                if let Some(arr) = pair.as_array() {
                    let timestamp = arr
                        .first()
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let line = arr
                        .get(1)
                        .and_then(|l| l.as_str())
                        .unwrap_or("")
                        .to_string();
                    entries.push(LogEntry {
                        timestamp,
                        line,
                        labels: labels.clone(),
                    });
                }
            }
        }
    }

    entries
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/logs", get(query_logs))
        .route("/logs/labels", get(labels))
        .route("/logs/labels/{name}/values", get(label_values))
}
