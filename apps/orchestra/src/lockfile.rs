//! PID-file based instance lock.

use anyhow::{Context, Result};
use std::path::Path;
use tracing::info;

pub fn acquire_lock(lockfile: &Path) -> Result<()> {
    if let Some(parent) = lockfile.parent() {
        std::fs::create_dir_all(parent).context("create lockfile directory")?;
    }

    if lockfile.exists()
        && let Ok(content) = std::fs::read_to_string(lockfile)
    {
        if let Ok(pid) = content.trim().parse::<u32>() {
            // Check if process is running
            use std::process::Command;
            if Command::new("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                anyhow::bail!("orchestra already running (PID {pid})");
            }
        }
        info!("removing stale lockfile");
    }

    std::fs::write(lockfile, std::process::id().to_string()).context("write lockfile")?;
    Ok(())
}
