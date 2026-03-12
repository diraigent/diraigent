use crate::config::Config;
use anyhow::Context;
use axum::Router;
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::{net::SocketAddr, time::Instant};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub use crate::logging::init_tracing;

/// CORS layer that denies all cross-origin requests.
///
/// Use this as the safe default when no explicit `CORS_ORIGINS` env var is configured.
/// Prefer [`dev_cors`] in local development environments.
pub fn standard_cors() -> CorsLayer {
    CorsLayer::new()
}

pub fn standard_trace()
-> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http().make_span_with(
        tower_http::trace::DefaultMakeSpan::new()
            .level(tracing::Level::INFO)
            .include_headers(false),
    )
}

pub fn dev_cors() -> CorsLayer {
    CorsLayer::permissive()
}

pub fn observability(
    config: &Config,
) -> anyhow::Result<opentelemetry_sdk::metrics::SdkMeterProvider> {
    let service_name = Box::leak(config.service_name.clone().into_boxed_str());
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(config.otlp_endpoint.clone())
        .with_timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create OTLP exporter: {}", e))?;

    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .build();

    global::set_meter_provider(meter_provider.clone());
    let meter = global::meter(service_name);
    let counter = meter.u64_counter("requests_total").build();
    counter.add(1, &[KeyValue::new("service", config.service_name.clone())]);
    tracing::info!("✅ Observability setup");
    Ok(meter_provider)
}

pub fn init_config_and_observability() -> anyhow::Result<(
    Config,
    opentelemetry_sdk::metrics::SdkMeterProvider,
    Instant,
)> {
    let start_time = Instant::now();
    let config = crate::config::config()?;
    let meter_provider = observability(&config)?;
    config.log_config();
    Ok((config, meter_provider, start_time))
}

pub async fn init_db_pool() -> anyhow::Result<crate::db::DbPool> {
    crate::db::create_pool()
        .await
        .context("Failed to create database pool")
}

pub fn log_startup_complete(addr: impl std::fmt::Display, start_time: Instant) {
    tracing::info!(
        addr = %addr,
        startup_time_ms = start_time.elapsed().as_millis(),
        "✅ Server ready"
    );
}

pub fn log_shutdown_complete() {
    tracing::info!("🛑 shutdown complete");
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to install CTRL+C handler: {}", e);
            return;
        }
        tracing::info!("Received CTRL+C signal");
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
                tracing::info!("Received SIGTERM signal");
            }
            Err(e) => {
                tracing::error!("Failed to install SIGTERM handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("🛑 Initiating graceful shutdown...");
}

pub async fn run(
    config: &Config,
    meter_provider: SdkMeterProvider,
    app: Router,
    start_time: Instant,
) -> anyhow::Result<()> {
    let addr: SocketAddr = config
        .bind_address()
        .parse()
        .with_context(|| format!("Invalid address: {}", config.bind_address()))?;

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind: {}", addr))?;

    log_startup_complete(addr, start_time);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Server failed")?;

    log_shutdown_complete();
    drop(meter_provider);
    Ok(())
}

pub async fn init_server() -> anyhow::Result<(
    Config,
    opentelemetry_sdk::metrics::SdkMeterProvider,
    crate::db::DbPool,
    Instant,
)> {
    init_tracing();
    let (config, meter_provider, start_time) = init_config_and_observability()?;
    let pool = init_db_pool().await?;

    Ok((config, meter_provider, pool, start_time))
}

pub fn init_server_stateless() -> anyhow::Result<(
    Config,
    opentelemetry_sdk::metrics::SdkMeterProvider,
    Instant,
)> {
    // Initialize tracing/logging first so config logs and observability messages are visible
    init_tracing();

    // Reuse existing helper that builds config and observability
    let (config, meter_provider, start_time) = init_config_and_observability()?;

    Ok((config, meter_provider, start_time))
}
