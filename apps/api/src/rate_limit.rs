use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    middleware::Next,
};
use dashmap::DashMap;
use std::{net::IpAddr, sync::LazyLock, time::Instant};
use uuid::Uuid;

use crate::AppState;

struct Entry {
    count: u32,
    window_start: Instant,
}

// ── IP-based limit ────────────────────────────────────────────────────────────
static IP_MAP: LazyLock<DashMap<IpAddr, Entry>> = LazyLock::new(DashMap::new);

// ── Per-agent limit ───────────────────────────────────────────────────────────
static AGENT_MAP: LazyLock<DashMap<Uuid, Entry>> = LazyLock::new(DashMap::new);

// ── Per-project limit ─────────────────────────────────────────────────────────
static PROJECT_MAP: LazyLock<DashMap<Uuid, Entry>> = LazyLock::new(DashMap::new);

// ── Per-tenant limit ──────────────────────────────────────────────────────────
static TENANT_MAP: LazyLock<DashMap<Uuid, Entry>> = LazyLock::new(DashMap::new);

/// Cached project_id → (tenant_id, rate_limit_per_min) mapping.
/// Entries are refreshed every 5 minutes to pick up limit changes.
struct TenantInfo {
    tenant_id: Uuid,
    rate_limit: u32,
    fetched_at: Instant,
}

static PROJECT_TENANT_CACHE: LazyLock<DashMap<Uuid, TenantInfo>> = LazyLock::new(DashMap::new);

/// TTL for project→tenant cache entries (seconds).
const TENANT_CACHE_TTL_SECS: u64 = 300;

const WINDOW_SECS: u64 = 60;

/// Rate limit ceilings, configurable via environment variables.
/// Parsed once on first use.
struct Limits {
    ip: u32,
    agent: u32,
    project: u32,
}

static LIMITS: LazyLock<Limits> = LazyLock::new(|| {
    fn env_or(key: &str, default: u32) -> u32 {
        std::env::var(key)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
    Limits {
        ip: env_or("RATE_LIMIT_IP", 600),
        agent: env_or("RATE_LIMIT_AGENT", 600),
        project: env_or("RATE_LIMIT_PROJECT", 1_000),
    }
});

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_ip(req: &Request<Body>) -> IpAddr {
    // Prefer the actual TCP peer address from ConnectInfo if available.
    // This cannot be spoofed by clients (unlike X-Forwarded-For / X-Real-IP).
    // Requires `into_make_service_with_connect_info::<SocketAddr>()` on the server.
    if let Some(connect_info) = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return connect_info.0.ip();
    }

    // Fallback: proxy headers. These are spoofable unless the reverse proxy (e.g.
    // nginx with `set_real_ip_from`) strips/overwrites them. When deploying behind
    // a trusted proxy, ensure the proxy is configured to set these headers and that
    // direct client connections are not accepted.
    if let Some(xff) = req.headers().get("x-forwarded-for")
        && let Ok(val) = xff.to_str()
        && let Some(first) = val.split(',').next()
        && let Ok(ip) = first.trim().parse()
    {
        return ip;
    }
    if let Some(xri) = req.headers().get("x-real-ip")
        && let Ok(val) = xri.to_str()
        && let Ok(ip) = val.trim().parse()
    {
        return ip;
    }
    // Fallback to loopback
    IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
}

/// Reads the `X-Agent-Id` header and parses it as a UUID.
fn extract_agent_id(req: &Request<Body>) -> Option<Uuid> {
    req.headers()
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Extracts the first UUID segment from the request path.
///
/// Routes like `/v1/{project_id}/tasks` carry the project UUID as
/// the first UUID segment after the prefix. We scan all path segments and return
/// the first one that is a valid UUID.
fn extract_project_id(req: &Request<Body>) -> Option<Uuid> {
    req.uri()
        .path()
        .split('/')
        .find_map(|seg| Uuid::parse_str(seg).ok())
}

/// Check (and consume) one slot in a generic `DashMap<K, Entry>` rate limiter.
/// Returns `true` if the request is allowed, `false` if it should be rejected.
fn check<K: std::hash::Hash + Eq + Copy>(
    map: &DashMap<K, Entry>,
    key: K,
    max: u32,
    now: Instant,
) -> bool {
    let mut entry = map.entry(key).or_insert(Entry {
        count: 0,
        window_start: now,
    });
    if now.duration_since(entry.window_start).as_secs() >= WINDOW_SECS {
        entry.count = 1;
        entry.window_start = now;
        true
    } else if entry.count < max {
        entry.count += 1;
        true
    } else {
        false
    }
}

/// Resolve project_id → (tenant_id, rate_limit_per_min), with caching.
/// Returns None if the project doesn't exist (skip tenant limit).
async fn resolve_tenant(state: &AppState, project_id: Uuid) -> Option<(Uuid, u32)> {
    let now = Instant::now();

    // Check cache
    if let Some(entry) = PROJECT_TENANT_CACHE.get(&project_id)
        && now.duration_since(entry.fetched_at).as_secs() < TENANT_CACHE_TTL_SECS
    {
        return Some((entry.tenant_id, entry.rate_limit));
    }

    // Cache miss — single lightweight query
    let row: Option<(Uuid, i32)> = sqlx::query_as(
        "SELECT p.tenant_id, t.rate_limit_per_min
         FROM diraigent.project p
         JOIN diraigent.tenant t ON p.tenant_id = t.id
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .ok()?;

    let (tenant_id, limit) = row?;
    let rate_limit = limit.max(0) as u32;

    PROJECT_TENANT_CACHE.insert(
        project_id,
        TenantInfo {
            tenant_id,
            rate_limit,
            fetched_at: now,
        },
    );

    Some((tenant_id, rate_limit))
}

fn rate_limited_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("content-type", "application/json")
        .header("retry-after", "60")
        .body(Body::from(r#"{"error":"rate limit exceeded"}"#))
        .unwrap()
}

// ── Middleware ────────────────────────────────────────────────────────────────

pub async fn rate_limit(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let now = Instant::now();

    // 1. IP-based limit
    let ip = extract_ip(&request);
    if !check(&IP_MAP, ip, LIMITS.ip, now) {
        crate::metrics::record_rate_limit();
        return rate_limited_response();
    }

    // 2. Per-agent-ID limit (only when header is present)
    if let Some(agent_id) = extract_agent_id(&request)
        && !check(&AGENT_MAP, agent_id, LIMITS.agent, now)
    {
        crate::metrics::record_rate_limit();
        return rate_limited_response();
    }

    // 3. Per-project-ID limit (derived from URL path)
    let project_id = extract_project_id(&request);
    if let Some(pid) = project_id
        && !check(&PROJECT_MAP, pid, LIMITS.project, now)
    {
        crate::metrics::record_rate_limit();
        return rate_limited_response();
    }

    // 4. Per-tenant limit (resolved from project → tenant, with per-tenant rate)
    if let Some(pid) = project_id
        && let Some((tenant_id, tenant_limit)) = resolve_tenant(&state, pid).await
        && tenant_limit > 0
        && !check(&TENANT_MAP, tenant_id, tenant_limit, now)
    {
        crate::metrics::record_rate_limit();
        return rate_limited_response();
    }

    next.run(request).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Uri;

    #[test]
    fn extract_project_id_from_path() {
        let project = Uuid::new_v4();
        let uri: Uri = format!("/v1/{project}/tasks").parse().unwrap();
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        assert_eq!(extract_project_id(&req), Some(project));
    }

    #[test]
    fn extract_project_id_none_for_no_uuid() {
        let req = Request::builder()
            .uri("/v1/agents")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_project_id(&req), None);
    }

    #[test]
    fn extract_agent_id_from_header() {
        let agent = Uuid::new_v4();
        let req = Request::builder()
            .header("x-agent-id", agent.to_string())
            .uri("/v1/heartbeat")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_agent_id(&req), Some(agent));
    }

    #[test]
    fn extract_agent_id_missing_header() {
        let req = Request::builder()
            .uri("/v1/heartbeat")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_agent_id(&req), None);
    }

    #[test]
    fn rate_check_allows_up_to_max() {
        let map: DashMap<u32, Entry> = DashMap::new();
        let now = Instant::now();
        for _ in 0..3 {
            assert!(check(&map, 42u32, 3, now));
        }
        assert!(!check(&map, 42u32, 3, now));
    }

    #[test]
    fn rate_check_resets_after_window() {
        let map: DashMap<u32, Entry> = DashMap::new();
        let old = Instant::now() - std::time::Duration::from_secs(61);
        // Manually insert an exhausted entry from an old window
        map.insert(
            99u32,
            Entry {
                count: 100,
                window_start: old,
            },
        );
        // Should be allowed — window has expired
        assert!(check(&map, 99u32, 3, Instant::now()));
    }
}
