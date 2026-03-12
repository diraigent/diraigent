use std::env;
use tracing_subscriber::prelude::*;

/// Initialize tracing with JSON output and optional Loki support.
///
/// Reads from env vars:
/// - `RUST_LOG` — filter (default: `info`)
/// - `LOG_FORMAT` — `json` (default) or `pretty` for human-readable dev output
/// - `LOKI_URL` — Loki base URL (optional, enables Loki push)
/// - `SERVICE_NAME` — label for Loki (default: `unknown_service`)
/// - `LOKI_ENV` — label for Loki (default: `dev`)
/// - `HOSTNAME` — label for Loki (default: `unknown`)
pub fn init_tracing() {
    dotenvy::dotenv().ok();

    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    let use_json = env::var("LOG_FORMAT")
        .map(|v| v != "pretty")
        .unwrap_or(true);

    if let Ok(loki_url) = env::var("LOKI_URL") {
        let service_name =
            env::var("SERVICE_NAME").unwrap_or_else(|_| "unknown_service".to_string());
        let loki_env = env::var("LOKI_ENV").unwrap_or_else(|_| "dev".to_string());
        let hostname = env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());

        // Strip any loki path suffix that may have been accidentally included in the env var.
        let loki_base = loki_url
            .trim_end_matches("/loki/api/v1/push")
            .trim_end_matches("/loki/api/v1")
            .trim_end_matches('/')
            .to_string();

        match tracing_loki::url::Url::parse(&loki_base) {
            Ok(url) => {
                match tracing_loki::builder()
                    .label("app", service_name)
                    .expect("valid label")
                    .label("env", loki_env)
                    .expect("valid label")
                    .label("host", hostname)
                    .expect("valid label")
                    .build_url(url)
                {
                    Ok((loki_layer, task)) => {
                        tokio::spawn(task);
                        if use_json {
                            let fmt_layer = tracing_subscriber::fmt::layer().json();
                            tracing_subscriber::registry()
                                .with(env_filter)
                                .with(fmt_layer)
                                .with(loki_layer)
                                .init();
                        } else {
                            let fmt_layer = tracing_subscriber::fmt::layer();
                            tracing_subscriber::registry()
                                .with(env_filter)
                                .with(fmt_layer)
                                .with(loki_layer)
                                .init();
                        }
                        tracing::info!("Loki logging enabled");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to build Loki layer: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("Invalid LOKI_URL: {e}");
            }
        }
    }

    // Fallback: stdout only
    if use_json {
        let fmt_layer = tracing_subscriber::fmt::layer().json();
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    } else {
        let fmt_layer = tracing_subscriber::fmt::layer();
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }
}
