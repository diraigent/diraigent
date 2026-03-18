use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{info, warn};

use super::{GIT_NET_TIMEOUT_SECS, WorktreeManager, unix_days_to_ymd};

impl WorktreeManager {
    /// Release: squash-merge a source branch into the default branch, tag, and push.
    ///
    /// Steps:
    /// 1. Checkout default branch and pull latest
    /// 2. Squash-merge source branch (default: "dev")
    /// 3. Commit with provided message
    /// 4. Create date-based tag (e.g. v20260313-01)
    /// 5. Push default branch + tag to all remotes
    ///
    /// Returns a summary message with the tag name.
    /// Release with an environment target.
    ///
    /// `release_env` controls the release mode:
    /// - `"production"` — squash-merge + changelog + tag + push
    /// - `"staging"` — squash-merge + push (no changelog, no tag)
    /// - anything else / None — squash-merge only (local)
    pub fn release(
        &self,
        source_branch: Option<&str>,
        message: Option<&str>,
        release_env: Option<&str>,
    ) -> Result<String> {
        let git_root = self
            .git_root
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("git_mode=none: release operations not supported"))?;

        // Release target is always "main" (production branch).
        // default_branch is the dev branch (e.g. "dev"), which is the default source.
        let target = "main";
        let source = source_branch.unwrap_or(&self.default_branch);

        // Generate date-based tag: v{YYYYMMDD}-{NN}
        let tag = self.generate_release_tag()?;

        // Check for .diraigent/ script-based release
        let config = crate::project::diraigent_config::load_config(git_root)?;
        let script_path = crate::project::diraigent_config::prepare_hook_script(
            git_root,
            "release",
            &config.release,
        )?;

        if let Some(script) = script_path {
            return self.run_release_script(
                &script,
                source,
                target,
                &tag,
                &config.release,
                release_env,
            );
        }

        // Fallback: built-in release logic (squash-merge)
        self.release_builtin(source, target, &tag, message)
    }

    /// Run a `.diraigent/release.sh` script with environment variables.
    fn run_release_script(
        &self,
        script: &Path,
        source: &str,
        target: &str,
        tag: &str,
        hook_config: &crate::project::diraigent_config::HookConfig,
        release_env: Option<&str>,
    ) -> Result<String> {
        let git_root = self.git_root.as_deref().unwrap();
        let env_label = release_env.unwrap_or("local");

        info!(
            script = %script.display(),
            source,
            target,
            tag,
            release_env = env_label,
            "running .diraigent/release.sh"
        );

        let mut cmd = Command::new("bash");
        cmd.arg(script);

        // Pass --production or --staging flag to the script
        if let Some(env) = release_env {
            cmd.arg(format!("--{env}"));
        }

        cmd.current_dir(git_root)
            .env("DIRAIGENT_PROJECT_PATH", git_root)
            .env("DIRAIGENT_BRANCH", source)
            .env("DIRAIGENT_TARGET_BRANCH", target)
            .env("DIRAIGENT_VERSION", tag)
            .env("DIRAIGENT_RELEASE_ENV", env_label)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Pass extra env vars from config
        for (k, v) in &hook_config.env {
            cmd.env(k, v);
        }

        let child = cmd.spawn().context("spawn release script")?;
        let output = child
            .wait_with_output()
            .context("wait for release script")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            let msg = if stderr.is_empty() {
                stdout.clone()
            } else {
                stderr.clone()
            };
            bail!("release script failed: {msg}");
        }

        if !stderr.is_empty() {
            warn!(stderr = %stderr.trim(), "release script stderr");
        }

        // Invalidate fetch cache
        *self.last_fetch.lock().unwrap() = None;

        // Return last non-empty line of stdout as the message
        let message = stdout
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("Release completed")
            .to_string();
        Ok(message)
    }

    /// Generate date-time based release tag: v{YYYYMMDD}-{HHMM}
    fn generate_release_tag(&self) -> Result<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = now / 86400;
        let (y, m, d) = unix_days_to_ymd(days as i64);
        let day_secs = now % 86400;
        let h = day_secs / 3600;
        let min = (day_secs % 3600) / 60;
        Ok(format!("v{y:04}{m:02}{d:02}-{h:02}{min:02}"))
    }

    /// Built-in release logic (original squash-merge implementation).
    fn release_builtin(
        &self,
        source: &str,
        target: &str,
        tag: &str,
        message: Option<&str>,
    ) -> Result<String> {
        // Verify source branch exists
        self.git(&["rev-parse", "--verify", source])
            .with_context(|| format!("source branch '{source}' does not exist"))?;

        // Checkout target (production) branch
        let current = self.git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if current.trim() != target {
            self.git(&["checkout", target])
                .with_context(|| format!("checkout {target} for release"))?;
        }

        // Pull latest if remote is configured
        if self
            .git_output(&["config", "--get", &format!("branch.{target}.remote")])
            .is_ok()
        {
            self.run_git(
                &["pull", "--rebase", "origin", target],
                GIT_NET_TIMEOUT_SECS,
            )
            .ok(); // Best-effort; may fail if no remote
        }

        // Check if there's anything to merge
        let diff_check =
            self.git_output(&["rev-list", "--count", &format!("{target}..{source}")])?;
        let commit_count: i32 = diff_check.trim().parse().unwrap_or(0);
        if commit_count == 0 {
            bail!("nothing to release: {source} has no new commits over {target}");
        }

        // Squash merge
        self.git(&["merge", "--squash", source])
            .with_context(|| format!("squash merge {source} into {target}"))?;

        // Build commit message from git log if not provided
        let commit_msg = if let Some(msg) = message {
            msg.to_string()
        } else {
            // Summarize commits being merged
            let log = self
                .git_output(&[
                    "log",
                    "--oneline",
                    "--no-merges",
                    &format!("{target}..{source}"),
                ])
                .unwrap_or_default();
            format!("release: squash merge {source} into {target}\n\n{log}")
        };

        self.git(&["commit", "-m", &commit_msg])
            .with_context(|| "commit squash merge")?;

        self.git(&["tag", tag])
            .with_context(|| format!("create tag {tag}"))?;

        // Push to all remotes
        let remotes_output = self.git_output(&["remote"]).unwrap_or_default();
        let remotes: Vec<&str> = remotes_output.lines().filter(|r| !r.is_empty()).collect();

        let mut push_results = Vec::new();
        for remote in &remotes {
            match self.run_git(&["push", remote, target], GIT_NET_TIMEOUT_SECS) {
                Ok(_) => push_results.push(format!("pushed {target} to {remote}")),
                Err(e) => push_results.push(format!("failed to push {target} to {remote}: {e}")),
            }
            match self.run_git(&["push", remote, tag], GIT_NET_TIMEOUT_SECS) {
                Ok(_) => push_results.push(format!("pushed {tag} to {remote}")),
                Err(e) => push_results.push(format!("failed to push {tag} to {remote}: {e}")),
            }
        }

        // Invalidate fetch cache
        *self.last_fetch.lock().unwrap() = None;

        Ok(format!(
            "Released {tag} ({commit_count} commits from {source})\n{}",
            push_results.join("\n")
        ))
    }
}
