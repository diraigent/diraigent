//! Orchestra configuration loaded from environment variables.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::crypto;
use crate::util;

pub struct Config {
    pub agent_id: String,
    /// Project ID — used only for WebSocket subscription scoping.
    /// Path resolution is always per-project at task spawn/reap time.
    pub project_id: Option<String>,
    pub diraigent_api: String,
    pub max_workers: usize,
    /// Base directory where all project repos live (PROJECTS_PATH env var, default ~/diraigent/projects).
    pub projects_path: PathBuf,
    pub poll_interval: u64,
    pub agent_cli: String,
    pub log_dir: PathBuf,
    pub lockfile: PathBuf,
    /// Default model for workers. If set, passed as `--model` to Claude CLI.
    /// Can be overridden per-task via task context `model` field.
    pub worker_model: Option<String>,
    /// Optional DEK for client-side decryption (passphrase-mode tenants).
    pub dek: Option<crypto::Dek>,
    /// Maximum number of failed implement cycles before a task is auto-cancelled.
    pub max_implement_cycles: u32,
}

pub type ActiveTasks = Arc<Mutex<HashMap<String, JoinHandle<()>>>>;

impl Config {
    pub fn from_env() -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        // Load .env files from all standard locations (walk-up, cwd, fallback).
        util::load_dotenv();

        // Sibling of binary — for deployed environments where .env ships alongside the binary.
        if let Some(ref d) = exe_dir {
            dotenvy::from_path(d.join(".env")).ok();
        }

        let agent_id = std::env::var("AGENT_ID").context("AGENT_ID not set")?;
        let project_id = std::env::var("PROJECT_ID").ok().filter(|s| !s.is_empty());
        let diraigent_api = std::env::var("DIRAIGENT_API_URL")
            .unwrap_or_else(|_| "http://localhost:8082/v1".into());
        let max_workers: usize = std::env::var("MAX_WORKERS")
            .unwrap_or_else(|_| "3".into())
            .parse()
            .unwrap_or(3);
        let poll_interval: u64 = std::env::var("POLL_INTERVAL")
            .unwrap_or_else(|_| "30".into())
            .parse()
            .unwrap_or(30);

        // Resolve agent-cli binary: prefer sibling of current exe, fall back to PATH
        let agent_cli = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("agent-cli")))
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "agent-cli".into());

        // Operational files live in cwd (e.g. /app in container), not inside the repo
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let log_dir = data_dir.join("logs");
        let lockfile = data_dir.join(".orchestra.pid");

        let worker_model = std::env::var("WORKER_MODEL").ok().filter(|s| !s.is_empty());

        let max_implement_cycles: u32 = std::env::var("MAX_IMPLEMENT_CYCLES")
            .unwrap_or_else(|_| "3".into())
            .parse()
            .unwrap_or(3);

        // Resolve DEK for client-side encryption/decryption.
        let dek = if let Ok(dek_b64) = std::env::var("DIRAIGENT_DEK") {
            match crypto::Dek::from_base64(&dek_b64) {
                Ok(dek) => {
                    tracing::info!("DEK loaded from DIRAIGENT_DEK env var");
                    Some(dek)
                }
                Err(e) => {
                    tracing::error!("invalid DIRAIGENT_DEK: {e}");
                    None
                }
            }
        } else if let (Ok(passphrase), Ok(salt), Ok(wrapped)) = (
            std::env::var("DIRAIGENT_PASSPHRASE"),
            std::env::var("DIRAIGENT_PASSPHRASE_SALT"),
            std::env::var("DIRAIGENT_WRAPPED_DEK"),
        ) {
            match crypto::derive_kek(&passphrase, &salt) {
                Ok(kek) => match crypto::Dek::unwrap(&wrapped, &kek) {
                    Ok(dek) => {
                        tracing::info!("DEK derived from DIRAIGENT_PASSPHRASE");
                        Some(dek)
                    }
                    Err(e) => {
                        tracing::error!("failed to unwrap DEK with passphrase-derived KEK: {e}");
                        None
                    }
                },
                Err(e) => {
                    tracing::error!("failed to derive KEK from passphrase: {e}");
                    None
                }
            }
        } else {
            if std::env::var("DIRAIGENT_PASSPHRASE").is_ok() {
                tracing::warn!(
                    "DIRAIGENT_PASSPHRASE is set but DIRAIGENT_PASSPHRASE_SALT and \
                     DIRAIGENT_WRAPPED_DEK are also required — set all three"
                );
            }
            None
        };

        // Base directory for all project repos.
        let projects_path = std::env::var("PROJECTS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| std::env::current_dir().unwrap());
                home.join("diraigent/projects")
            });

        Ok(Config {
            agent_id,
            project_id,
            diraigent_api,
            max_workers,
            projects_path,
            poll_interval,
            agent_cli,
            log_dir,
            lockfile,
            worker_model,
            dek,
            max_implement_cycles,
        })
    }
}
