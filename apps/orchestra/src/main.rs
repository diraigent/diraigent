// Modules are shared between the `orchestra` and `agent-cli` binaries.
// Each binary uses a different subset, so unused items are expected.
#![allow(dead_code)]

mod config;
mod constants;
mod crypto;
mod db;
mod engine;
mod git;
mod handlers;
mod indexer;
mod lockfile;
mod log_monitor;
mod project;
mod providers;
mod repo_decisions;
mod repo_knowledge;
mod repo_observations;
mod sync;
mod task_id;
mod util;
mod ws;

use anyhow::Result;
use config::{ActiveTasks, Config};
use project::api::ProjectsApi;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<()> {
    // Install the default rustls crypto provider before any TLS connections.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls CryptoProvider");

    // Check for `orchestra run <file>` subcommand before full init.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "run" {
        return run_headless(&args[2..]).await;
    }

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

    // Init task source based on orchestration mode.
    let local_db;
    let task_source: Arc<dyn engine::task_source::TaskSource>;

    if config.orchestration_mode == config::OrchestrationMode::Local {
        let db = db::open(&config.data_dir)?;
        info!("orchestration_mode=local — state machine owned by orchestra");
        local_db = Some(db.clone());
        task_source = Arc::new(engine::orchestra_source::OrchestraTaskSource::new(
            db,
            api.clone(),
        ));
    } else {
        local_db = None;
        task_source = Arc::new(api.clone());
    };

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

    // Start background sync loop (local mode only)
    if let Some(ref db) = local_db {
        sync::spawn(db.clone(), Arc::new(api.clone()), shutdown.clone());
    }

    // Report orchestra version in agent metadata
    {
        let version = env!("CARGO_PKG_VERSION");
        match api.get_agent(&config.agent_id).await {
            Ok(agent) => {
                let mut meta = agent["metadata"].clone();
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert("version".into(), serde_json::json!(version));
                } else {
                    meta = serde_json::json!({"runtime": "orchestra", "version": version});
                }
                if let Err(e) = api
                    .update_agent(&config.agent_id, &serde_json::json!({"metadata": meta}))
                    .await
                {
                    warn!("failed to report version in agent metadata: {e}");
                }
            }
            Err(e) => warn!("failed to fetch agent for version update: {e}"),
        }
    }

    info!("listening (workers={})", config.max_workers);

    let active: ActiveTasks = Arc::new(Mutex::new(HashMap::new()));
    let lock_queue: config::LockQueue = Arc::new(Mutex::new(HashMap::new()));

    // A polling-only worker can leave chat and git requests to another agent.
    if !matches!(std::env::var("DISABLE_WS").as_deref(), Ok("1" | "true")) {
        let api_url = config.diraigent_api.clone();
        let agent_id = config.agent_id.clone();
        let ws_api = api.clone();
        let ws_pp = config.projects_path.clone();
        let ws_shutdown = shutdown.clone();
        tokio::spawn(async move {
            ws::run_ws_loop(&api_url, &agent_id, ws_api, ws_pp, ws_shutdown).await;
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
    {
        let projects = scoped_projects(
            api.list_projects().await.unwrap_or_default(),
            config.project_id.as_deref(),
        );
        engine::spawner::poll_ready_tasks_with_projects(
            &task_source,
            &config,
            &active,
            &lock_queue,
            &projects,
        )
        .await;
    }

    // Main polling loop
    let mut last_poll = std::time::Instant::now();
    let mut last_heartbeat = std::time::Instant::now();
    let mut last_index = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(config.indexer_interval))
        .unwrap_or_else(std::time::Instant::now);
    if config.indexer_interval > 0 {
        info!("indexer enabled (interval={}s)", config.indexer_interval);
    }
    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Reap finished tasks — returns true if file locks were released.
        let locks_released = engine::scheduler::reap_finished(
            task_source.as_ref(),
            &config.projects_path,
            &active,
            &lock_queue,
        )
        .await;

        // Heartbeat every 60s
        if last_heartbeat.elapsed().as_secs() >= 60 {
            if let Err(e) = api.heartbeat().await {
                warn!("heartbeat failed: {e}");
            }
            last_heartbeat = std::time::Instant::now();
        }

        // Poll on interval, or immediately when file locks were released (queue drain).
        if locks_released || last_poll.elapsed().as_secs() >= config.poll_interval {
            // Fetch projects once and share across spawner + work item processing.
            let projects = scoped_projects(
                api.list_projects().await.unwrap_or_default(),
                config.project_id.as_deref(),
            );
            engine::spawner::poll_ready_tasks_with_projects(
                &task_source,
                &config,
                &active,
                &lock_queue,
                &projects,
            )
            .await;
            if !matches!(
                std::env::var("DISABLE_WORK_ITEMS").as_deref(),
                Ok("1" | "true")
            ) {
                process_ready_work_items(&api, &projects).await;
            }
            last_poll = std::time::Instant::now();
        }

        // Run indexer on its own interval (0 = disabled).
        if config.indexer_interval > 0 && last_index.elapsed().as_secs() >= config.indexer_interval
        {
            indexer::tick(&api, &config.projects_path, config.project_id.as_deref()).await;
            last_index = std::time::Instant::now();
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
        engine::scheduler::reap_finished(
            task_source.as_ref(),
            &config.projects_path,
            &active,
            &lock_queue,
        )
        .await;
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

fn scoped_projects(projects: Vec<Value>, project_id: Option<&str>) -> Vec<Value> {
    match project_id {
        Some(project_id) => projects
            .into_iter()
            .filter(|project| project["id"].as_str() == Some(project_id))
            .collect(),
        None => projects,
    }
}

/// Poll all projects for work items in 'ready' status and create tasks from them.
///
/// For each ready work item, the function:
/// 1. Transitions the work item to 'processing'
/// 2. Creates task(s) based on the work item's intent_type
/// 3. Links the created task to the work item
/// 4. Transitions the work item to 'active'
///
/// Errors on individual work items are logged but do not stop batch processing.
async fn process_ready_work_items(api: &ProjectsApi, projects: &[Value]) {
    for project in projects {
        let project_id = match project["id"].as_str() {
            Some(id) => id,
            None => continue,
        };

        let work_items = match api.list_work_items_by_status(project_id, "ready").await {
            Ok(w) => w,
            Err(e) => {
                warn!("work: failed to fetch ready work items for project {project_id}: {e}");
                continue;
            }
        };

        for work_item in &work_items {
            if let Err(e) = process_single_work_item(api, project_id, work_item, project).await {
                let work_id = work_item["id"].as_str().unwrap_or("unknown");
                error!("work: failed to process work item {work_id}: {e:#}");
            }
        }
    }
}

/// Process a single work item: create the appropriate task and link it.
async fn process_single_work_item(
    api: &ProjectsApi,
    project_id: &str,
    work_item: &Value,
    _project: &Value,
) -> Result<()> {
    let work_id = work_item["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("work item missing id"))?;
    let work_title = work_item["title"].as_str().unwrap_or("Untitled work item");
    let work_description = work_item["description"].as_str().unwrap_or("");
    let intent_type = work_item["intent_type"].as_str().unwrap_or("complex");

    // 1. Transition work item to 'processing'
    api.update_work_item_status(work_id, "processing").await?;

    // 2. Determine task parameters based on intent_type
    let (kind, urgent, decompose) = match intent_type {
        "simple" => ("feature", false, false),
        "hotfix" => ("bug", true, false),
        "investigation" => ("research", false, false),
        "refactor" => ("refactor", false, false),
        _ => ("feature", false, true),
    };

    // 3. Build task body
    let mut context = serde_json::json!({
        "spec": work_description,
    });
    if decompose {
        context["decompose"] = serde_json::json!(true);
    }

    let mut task_body = serde_json::json!({
        "title": work_title,
        "kind": kind,
        "context": context,
    });
    if kind == "research" {
        task_body["context"]["mode"] = serde_json::json!("research");
    }
    if urgent {
        task_body["urgent"] = serde_json::json!(true);
    }

    // 4. Create the task
    let created_task = api.create_task(project_id, &task_body).await?;
    let task_id = created_task["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("created task missing id"))?;

    // 5. Link the task to the work item
    api.link_task_to_work_item(work_id, task_id).await?;

    // Linking to processing work queues the task atomically in the API, so a
    // worker can resolve the work item's Git policy before claiming it.

    // 6. Transition work item to 'active'
    api.update_work_item_status(work_id, "active").await?;

    info!(
        "Processed work item {} ({}): created task {}",
        work_id, intent_type, task_id
    );

    Ok(())
}

// ── Headless mode ─────────────────────────────────────────────────

/// Run a single work file without an API connection.
///
/// Usage: `orchestra run <file> [--project-path <path>]`
async fn run_headless(args: &[String]) -> Result<()> {
    use engine::local_source::LocalTaskSource;
    use engine::task_source::TaskSource;

    // Init tracing (simple, no Loki)
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Parse args
    if args.is_empty() {
        anyhow::bail!("usage: orchestra run <file.yaml> [--project-path <path>]");
    }
    let file_path = std::path::Path::new(&args[0]);
    let mut project_path = std::env::current_dir()?;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--project-path" | "-p" => {
                i += 1;
                if i < args.len() {
                    project_path = std::path::PathBuf::from(&args[i]);
                }
            }
            other => {
                anyhow::bail!("unknown argument: {other}");
            }
        }
        i += 1;
    }

    let source = LocalTaskSource::from_file(file_path, &project_path)?;
    let task_ids = source.task_ids().to_vec();
    let project_id = source.project_id().to_string();
    let repo_root = source.project_path().to_path_buf();

    if task_ids.is_empty() {
        info!("no tasks to run");
        return Ok(());
    }

    info!(
        "headless: {} task(s), project_path={}",
        task_ids.len(),
        repo_root.display()
    );

    let source: Arc<dyn TaskSource> = Arc::new(source);

    // Determine agent-cli path and log dir
    let agent_cli = std::env::var("AGENT_CLI").unwrap_or_else(|_| "agent-cli".to_string());
    let log_dir = std::env::temp_dir().join("orchestra-run");
    std::fs::create_dir_all(&log_dir).ok();

    let worker_model = std::env::var("WORKER_MODEL").ok();

    // Process each task sequentially
    for task_id in &task_ids {
        let tid = task_id::TaskId::new(task_id);
        info!("headless: starting task {tid}");

        // Claim
        if let Err(e) = source.claim_task(task_id).await {
            error!("headless: failed to claim {tid}: {e}");
            continue;
        }

        let task_data = source.get_task(task_id).await.ok();
        let step_name = task_data
            .as_ref()
            .and_then(|t| t["context"]["mode"].as_str())
            .unwrap_or("working");
        let worker_options = task_data.as_ref().and_then(|t| t["context"].get("worker"));

        let task_config = engine::worker::TaskConfig::for_mode(
            step_name,
            worker_options,
            None,
            worker_model.as_deref(),
        );

        info!(
            "headless: step={step_name} model={}",
            task_config.model.as_deref().unwrap_or("default")
        );

        // Create worktree manager
        let wm = if repo_root.join(".git").exists() {
            git::WorktreeManager::with_branch(&repo_root, "main")
        } else {
            git::WorktreeManager::disabled(&repo_root)
        };

        // Run worker
        match engine::worker::run_worker(
            source.as_ref(),
            &wm,
            task_id,
            &project_id,
            &repo_root,
            &agent_cli,
            &log_dir,
            &task_config,
            None,  // no encryption in headless mode
            false, // don't upload logs
            false, // don't store diffs
        )
        .await
        {
            Ok(result) => {
                info!(
                    "headless: task {tid} done — cost=${:.4} tokens={}in/{}out duration={}s",
                    result.cost_usd,
                    result.input_tokens,
                    result.output_tokens,
                    result.duration_seconds,
                );
                let next_state = if result.is_error {
                    "human_review"
                } else {
                    "done"
                };
                if let Err(e) = source.transition_task(task_id, next_state).await {
                    warn!("headless: failed to transition {tid} to {next_state}: {e}");
                }
            }
            Err(e) => {
                error!("headless: task {tid} failed: {e:#}");
                if let Err(te) = source.transition_task(task_id, "cancelled").await {
                    warn!("headless: failed to cancel {tid}: {te}");
                }
            }
        }
    }

    info!("headless: all tasks complete");
    Ok(())
}

#[cfg(test)]
mod project_scope_tests {
    use super::scoped_projects;

    #[test]
    fn selected_project_limits_polling() {
        let projects = vec![
            serde_json::json!({"id": "diraigent"}),
            serde_json::json!({"id": "stareto"}),
        ];
        assert_eq!(
            scoped_projects(projects.clone(), Some("diraigent")),
            vec![projects[0].clone()]
        );
        assert_eq!(scoped_projects(projects.clone(), None), projects);
    }
}
