use axum::body::Body;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::metrics::{Counter, Histogram};
use std::sync::LazyLock;
use std::time::Instant;

static HTTP_REQUESTS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .u64_counter("http.server.request.count")
        .with_description("Total HTTP requests")
        .build()
});

static HTTP_DURATION: LazyLock<Histogram<f64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .f64_histogram("http.server.request.duration")
        .with_description("HTTP request duration in seconds")
        .with_unit("s")
        .build()
});

static AUTH_FAILURES: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .u64_counter("auth.failure.count")
        .with_description("Authentication/authorization failures")
        .build()
});

static RATE_LIMIT_HITS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .u64_counter("rate_limit.hit.count")
        .with_description("Rate limit rejections")
        .build()
});

static WEBHOOK_DELIVERIES: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .u64_counter("webhook.delivery.count")
        .with_description("Webhook delivery attempts")
        .build()
});

static TASK_TRANSITIONS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .u64_counter("task.transition.count")
        .with_description("Task state transitions")
        .build()
});

static STALE_TASKS_DETECTED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter("diraigent-api")
        .u64_counter("task.stale.detected.count")
        .with_description("Stale tasks detected by heartbeat timeout")
        .build()
});

/// Middleware that records HTTP request count and latency.
pub async fn record_metrics(request: Request<Body>, next: Next) -> Response<Body> {
    let method = request.method().to_string();
    let path = normalize_path(request.uri().path());
    let start = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    let attrs = [
        KeyValue::new("http.method", method),
        KeyValue::new("http.route", path),
        KeyValue::new("http.status_code", status),
    ];

    HTTP_REQUESTS.add(1, &attrs);
    HTTP_DURATION.record(duration, &attrs);

    response
}

pub fn record_auth_failure(reason: &str) {
    AUTH_FAILURES.add(1, &[KeyValue::new("reason", reason.to_string())]);
}

pub fn record_rate_limit() {
    RATE_LIMIT_HITS.add(1, &[]);
}

pub fn record_webhook_delivery(success: bool) {
    WEBHOOK_DELIVERIES.add(
        1,
        &[KeyValue::new(
            "outcome",
            if success { "success" } else { "failure" },
        )],
    );
}

pub fn record_stale_tasks_detected(count: u64) {
    STALE_TASKS_DETECTED.add(count, &[]);
}

pub fn record_task_transition(from: &str, to: &str) {
    TASK_TRANSITIONS.add(
        1,
        &[
            KeyValue::new("from_state", from.to_string()),
            KeyValue::new("to_state", to.to_string()),
        ],
    );
}

/// Normalize path to replace UUIDs with `:id` for metric cardinality control.
fn normalize_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = segments
        .iter()
        .map(|s| {
            if s.len() == 36 && s.chars().filter(|c| *c == '-').count() == 4 {
                ":id".to_string()
            } else {
                s.to_string()
            }
        })
        .collect();
    normalized.join("/")
}
