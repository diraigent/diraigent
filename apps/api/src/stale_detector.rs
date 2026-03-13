use crate::db::DiraigentDb;
use crate::webhooks;
use serde::Serialize;
use serde_json::json;
use std::env;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_TIMEOUT_SECONDS: i64 = 600;
const SCAN_INTERVAL_SECONDS: u64 = 60;

/// How often to run the observation retention cleanup (every hour).
const OBSERVATION_CLEANUP_INTERVAL_SECONDS: u64 = 3600;

/// Default number of days to retain observations when not configured per-project.
const DEFAULT_OBSERVATION_RETENTION_DAYS: i32 = 30;

/// Default inactivity threshold before an idle/working agent is marked offline.
/// Heartbeat interval is 60s, so 3× gives two missed heartbeats of slack.
/// Override via `AGENT_OFFLINE_THRESHOLD_SECONDS` environment variable.
const DEFAULT_AGENT_OFFLINE_THRESHOLD_SECONDS: i64 = 180;

/// Default inactivity threshold before an agent is auto-revoked.
/// Override via `STALE_AGENT_THRESHOLD_DAYS` environment variable.
const DEFAULT_STALE_AGENT_THRESHOLD_DAYS: i64 = 7;

#[derive(Debug, Serialize)]
struct StaleTaskPayload {
    task_id: uuid::Uuid,
    task_title: String,
    task_state: String,
    agent_id: uuid::Uuid,
    agent_name: String,
    agent_last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    claimed_at: Option<chrono::DateTime<chrono::Utc>>,
    timeout_seconds: i64,
    action: String,
}

pub fn spawn_stale_detector(db: Arc<dyn DiraigentDb>, webhooks: webhooks::WebhookDispatcher) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;

        let stale_agent_threshold_days = env::var("STALE_AGENT_THRESHOLD_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|&d| d > 0)
            .unwrap_or(DEFAULT_STALE_AGENT_THRESHOLD_DAYS);

        let agent_offline_threshold_seconds = env::var("AGENT_OFFLINE_THRESHOLD_SECONDS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|&s| s > 0)
            .unwrap_or(DEFAULT_AGENT_OFFLINE_THRESHOLD_SECONDS);

        tracing::info!(
            interval_s = SCAN_INTERVAL_SECONDS,
            default_timeout_s = DEFAULT_TIMEOUT_SECONDS,
            agent_threshold_days = stale_agent_threshold_days,
            agent_offline_threshold_s = agent_offline_threshold_seconds,
            "Stale detector started",
        );

        let mut interval = tokio::time::interval(Duration::from_secs(SCAN_INTERVAL_SECONDS));
        loop {
            interval.tick().await;
            if let Err(e) = scan_stale_tasks(db.as_ref(), &webhooks).await {
                tracing::warn!(error = %e, "Stale task scan failed");
            }
            if let Err(e) = scan_inactive_agents(db.as_ref(), agent_offline_threshold_seconds).await
            {
                tracing::warn!(error = %e, "Inactive agent scan failed");
            }
            if let Err(e) = scan_stale_agents(db.as_ref(), stale_agent_threshold_days).await {
                tracing::warn!(error = %e, "Stale agent scan failed");
            }
        }
    });
}

async fn scan_stale_tasks(
    db: &dyn DiraigentDb,
    webhooks: &webhooks::WebhookDispatcher,
) -> anyhow::Result<()> {
    let stale_tasks = db.query_stale_tasks(DEFAULT_TIMEOUT_SECONDS).await?;

    if stale_tasks.is_empty() {
        return Ok(());
    }

    tracing::info!(count = stale_tasks.len(), "Detected stale tasks");
    crate::metrics::record_stale_tasks_detected(stale_tasks.len() as u64);

    for stale in &stale_tasks {
        let action = if stale.auto_release {
            "released"
        } else {
            "flagged"
        };

        tracing::warn!(
            task_id = %stale.task_id,
            agent_id = %stale.agent_id,
            agent = %stale.agent_name,
            last_seen = ?stale.agent_last_seen_at,
            action,
            "Stale task detected"
        );

        if stale.auto_release {
            let released = db.release_stale_task_conditional(stale.task_id).await?;
            if released {
                tracing::info!(task_id = %stale.task_id, "Stale task auto-released to ready");
                crate::metrics::record_task_transition(&stale.task_state, "ready");
            } else {
                tracing::debug!(task_id = %stale.task_id, "Task already transitioned, skipping release");
                continue;
            }
        }

        let _ = db.mark_agent_offline(stale.agent_id).await;

        let timeout_seconds = db.get_project_timeout(stale.project_id).await;
        let payload = StaleTaskPayload {
            task_id: stale.task_id,
            task_title: stale.task_title.clone(),
            task_state: stale.task_state.clone(),
            agent_id: stale.agent_id,
            agent_name: stale.agent_name.clone(),
            agent_last_seen_at: stale.agent_last_seen_at,
            claimed_at: stale.claimed_at,
            timeout_seconds,
            action: action.to_string(),
        };

        // Audit log
        let summary = format!("task.stale_timeout on task {}", stale.task_id);
        let payload_value = json!(payload);
        let _ = db
            .create_audit_entry(
                stale.project_id,
                None,
                None,
                "task.stale_timeout",
                "task",
                stale.task_id,
                &summary,
                None,
                Some(&payload_value),
            )
            .await;

        // Webhook dispatch
        webhooks.fire(stale.project_id, "task.stale_timeout", payload_value);
    }

    Ok(())
}

async fn scan_inactive_agents(db: &dyn DiraigentDb, threshold_seconds: i64) -> anyhow::Result<()> {
    let marked_offline = db.mark_inactive_agents_offline(threshold_seconds).await?;

    if marked_offline.is_empty() {
        return Ok(());
    }

    tracing::info!(
        count = marked_offline.len(),
        threshold_seconds,
        "Marked agents offline due to missed heartbeats",
    );

    for agent_id in &marked_offline {
        tracing::info!(
            %agent_id,
            threshold_seconds,
            "Agent marked offline: no heartbeat within threshold",
        );
    }

    Ok(())
}

/// Spawns a background task that periodically deletes observations older than
/// each project's configured `observation_retention_days` (default: 30 days).
pub fn spawn_observation_cleaner(db: Arc<dyn DiraigentDb>) {
    tokio::spawn(async move {
        // Stagger the first run to avoid thundering herd at startup.
        tokio::time::sleep(Duration::from_secs(30)).await;

        let retention_days = env::var("OBSERVATION_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|&d| d > 0)
            .unwrap_or(DEFAULT_OBSERVATION_RETENTION_DAYS);

        tracing::info!(
            interval_s = OBSERVATION_CLEANUP_INTERVAL_SECONDS,
            default_retention_days = retention_days,
            "Observation retention cleaner started",
        );

        let mut interval =
            tokio::time::interval(Duration::from_secs(OBSERVATION_CLEANUP_INTERVAL_SECONDS));
        loop {
            interval.tick().await;
            match db
                .delete_old_observations_all_projects(retention_days)
                .await
            {
                Ok(0) => {}
                Ok(deleted) => {
                    tracing::info!(deleted, retention_days, "Deleted old observations");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Observation retention cleanup failed");
                }
            }
        }
    });
}

async fn scan_stale_agents(db: &dyn DiraigentDb, threshold_days: i64) -> anyhow::Result<()> {
    let revoked = db.revoke_stale_agents(threshold_days).await?;

    if revoked.is_empty() {
        return Ok(());
    }

    tracing::warn!(
        count = revoked.len(),
        threshold_days,
        "Auto-revoked stale agents due to heartbeat inactivity",
    );

    for agent_id in &revoked {
        tracing::warn!(
            %agent_id,
            threshold_days,
            "Agent revoked: no heartbeat within inactivity threshold",
        );
    }

    Ok(())
}
