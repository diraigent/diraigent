#![allow(dead_code)]

mod api;
mod chat;
mod config;
mod constants;
mod context;
mod crypto;
mod git;
mod git_handler;
mod git_provisioner;
mod git_strategy;
mod lockfile;
mod log_monitor;
mod pipeline;
mod project_paths;
mod prompt;
mod scheduler;
mod spawner;
mod step_profile;
mod task_id;
mod util;
mod worker;
mod ws_client;
mod ws_protocol;

use anyhow::Result;
use api::ProjectsApi;
use config::{ActiveTasks, Config};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tracing::{info, warn};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<()> {
    // Install the default rustls crypto provider before any TLS connections.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls CryptoProvider");

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tracing_loki=warn".into());

    let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);

    // Loki layer — enabled when LOKI_URL is set
    // LOKI_URL should be the base URL (e.g. http://host:3100) — tracing-loki appends /loki/api/v1/push
    let loki_layer = if let Ok(loki_url) = std::env::var("LOKI_URL") {
        match url::Url::parse(&loki_url) {
            Ok(url) => {
                let loki_env = std::env::var("LOKI_ENV").unwrap_or_else(|_| "dev".into());
                let (layer, task) = tracing_loki::builder()
                    .label("service", "orchestra")
                    .unwrap()
                    .label("component", "diraigent")
                    .unwrap()
                    .label("env", loki_env.as_str())
                    .unwrap()
                    .build_url(url)
                    .expect("valid Loki URL");

                // Spawn background task that ships logs to Loki
                tokio::spawn(task);
                tracing::info!("Loki logging enabled: {loki_url}");
                // Filter out tracing_loki's own logs to prevent feedback loops
                let loki_filter = tracing_subscriber::filter::FilterFn::new(|meta| {
                    meta.target() != "tracing_loki"
                });
                Some(layer.with_filter(loki_filter))
            }
            Err(e) => {
                eprintln!("Invalid LOKI_URL: {e}");
                None
            }
        }
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(loki_layer)
        .init();

    let config = Config::from_env()?;
    let api = ProjectsApi::new(&config.diraigent_api, &config.agent_id);

    info!(
        "projects_path={} project_id={}",
        config.projects_path.display(),
        config
            .project_id
            .as_deref()
            .unwrap_or("(all — tenant-scoped)")
    );

    // Instance lock
    lockfile::acquire_lock(&config.lockfile)?;

    // Shutdown flag — set by signal handler, checked by main loop
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();

    // Handle both SIGINT (Ctrl+C) and SIGTERM (Docker/systemd stop)
    tokio::spawn(async move {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate()).expect("register SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
        shutdown_signal.store(true, Ordering::SeqCst);
        info!("shutdown signal received — press Ctrl+C again to force exit");

        // Second signal: force-exit immediately
        let mut sigterm2 = signal(SignalKind::terminate()).expect("register SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm2.recv() => {},
        }
        warn!("force exit");
        std::process::exit(1);
    });

    info!("listening (workers={})", config.max_workers);

    let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));
    let lock_queue: config::LockQueue = Arc::new(Mutex::new(HashMap::new()));

    // Spawn WebSocket client loop (handles chat + git requests)
    {
        let api_url = config.diraigent_api.clone();
        let agent_id = config.agent_id.clone();
        let ws_api = api.clone();
        let ws_pp = config.projects_path.clone();
        let ws_shutdown = shutdown.clone();
        tokio::spawn(async move {
            ws_client::run_ws_loop(&api_url, &agent_id, ws_api, ws_pp, ws_shutdown).await;
        });
    }

    // Spawn log monitors for projects with service_name (when LOKI_URL is set)
    if let Ok(loki_url) = std::env::var("LOKI_URL") {
        let loki_label = std::env::var("LOKI_LABEL").unwrap_or_else(|_| "service".into());
        let api_monitor = api.clone();
        let shutdown_monitor = shutdown.clone();
        tokio::spawn(async move {
            log_monitor::spawn_log_monitors(&api_monitor, loki_url, loki_label, shutdown_monitor)
                .await;
        });
    }

    // Initial poll
    spawner::poll_ready_tasks(&api, &config, &active, &lock_queue).await;

    // Main polling loop
    let mut last_poll = std::time::Instant::now();
    let mut last_heartbeat = std::time::Instant::now();
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Reap finished tasks — returns true if file locks were released.
        let locks_released =
            scheduler::reap_finished(&api, &config.projects_path, &active, &lock_queue).await;

        // Heartbeat every 60s
        if last_heartbeat.elapsed().as_secs() >= 60 {
            if let Err(e) = api.heartbeat().await {
                warn!("heartbeat failed: {e}");
            }
            last_heartbeat = std::time::Instant::now();
        }

        // Poll on interval, or immediately when file locks were released (queue drain).
        if locks_released || last_poll.elapsed().as_secs() >= config.poll_interval {
            spawner::poll_ready_tasks(&api, &config, &active, &lock_queue).await;
            last_poll = std::time::Instant::now();
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    // --- Graceful shutdown ---
    info!("shutting down...");

    // Kill child processes (script/claude) via process group, but ignore SIGTERM for ourselves
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
        libc::kill(0, libc::SIGTERM);
    }

    // Wait for active workers to finish (children should exit from SIGTERM)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        scheduler::reap_finished(&api, &config.projects_path, &active, &lock_queue).await;
        if active.lock().await.is_empty() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let remaining = active.lock().await.len();
            warn!("shutdown deadline exceeded, {remaining} workers still active");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    std::fs::remove_file(&config.lockfile).ok();
    info!("shutdown complete");
    Ok(())
}
