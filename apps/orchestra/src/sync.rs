//! Background sync loop: pushes local state to the API so the web/TUI can display it.
//!
//! Runs on a fixed interval, collects unsynced task states, updates, and changed files
//! from the local SQLite, batches them into a single `POST /v1/orchestra/sync` call,
//! and marks rows as synced on success.

use crate::db::{self, Db};
use crate::project::api::ProjectsApi;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, error, warn};

/// Default sync interval in seconds.
const SYNC_INTERVAL_SECS: u64 = 10;

/// Spawn the background sync loop as a tokio task.
pub fn spawn(db: Db, api: Arc<ProjectsApi>, shutdown: Arc<AtomicBool>) {
    tokio::spawn(async move {
        run_loop(db, api, shutdown).await;
    });
}

async fn run_loop(db: Db, api: Arc<ProjectsApi>, shutdown: Arc<AtomicBool>) {
    let interval = std::time::Duration::from_secs(SYNC_INTERVAL_SECS);
    loop {
        tokio::time::sleep(interval).await;
        if shutdown.load(Ordering::SeqCst) {
            // Final flush before exit
            if let Err(e) = sync_once(&db, &api).await {
                warn!("sync: final flush failed: {e}");
            }
            break;
        }
        if let Err(e) = sync_once(&db, &api).await {
            error!("sync: {e}");
        }
    }
}

/// Run one sync cycle: collect unsynced data, push to API, mark as synced.
async fn sync_once(db: &Db, api: &ProjectsApi) -> anyhow::Result<()> {
    // 1. Collect unsynced task states
    let task_states = db::task_execution::get_unsynced(db)?;
    // 2. Collect unsynced updates
    let updates = db::task_updates::get_unsynced(db)?;
    // 3. Collect unsynced changed files
    let changed_files = db::task_updates::get_unsynced_changed_files(db)?;

    let total = task_states.len() + updates.len() + changed_files.len();
    if total == 0 {
        return Ok(());
    }

    debug!(
        "sync: {} states, {} updates, {} files",
        task_states.len(),
        updates.len(),
        changed_files.len()
    );

    // Build sync batch
    let batch = serde_json::json!({
        "task_states": task_states,
        "task_updates": updates,
        "changed_files": changed_files,
    });

    // Push to API
    match api.post_sync_batch(&batch).await {
        Ok(_) => {
            // Mark everything as synced
            let state_ids: Vec<String> = task_states
                .iter()
                .filter_map(|v| v["task_id"].as_str().map(|s| s.to_string()))
                .collect();
            db::task_execution::mark_synced(db, &state_ids)?;

            let update_ids: Vec<String> = updates
                .iter()
                .filter_map(|v| v["id"].as_str().map(|s| s.to_string()))
                .collect();
            db::task_updates::mark_synced(db, &update_ids)?;

            let file_ids: Vec<String> = changed_files
                .iter()
                .filter_map(|v| v["id"].as_str().map(|s| s.to_string()))
                .collect();
            db::task_updates::mark_changed_files_synced(db, &file_ids)?;

            debug!("sync: pushed {total} items");
        }
        Err(e) => {
            warn!("sync: API push failed (will retry): {e}");
        }
    }
    Ok(())
}
