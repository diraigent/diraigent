use crate::task_id::TaskId;
use anyhow::{Context, Result, bail};
use std::sync::atomic::Ordering;
use tracing::{error, info, warn};

use super::{GIT_NET_TIMEOUT_SECS, WorktreeManager};

impl WorktreeManager {
    /// Maximum number of push retries when concurrent remote changes cause push to fail.
    const PUSH_RETRIES: usize = 3;

    /// Merge a task branch into the default branch.
    ///
    /// Convenience wrapper around `merge_to_branch` using `self.default_branch`.
    pub fn merge_to_main(&self, task_id: &str) -> Result<()> {
        let target = self.default_branch.clone();
        self.merge_to_branch(task_id, &target)
    }

    /// Merge a task branch into a specific target branch.
    ///
    /// Used by git strategies: `merge` targets the default or configured branch,
    /// `feature_branch` targets the work branch.
    pub fn merge_to_branch(&self, task_id: &str, target_branch: &str) -> Result<()> {
        if self.git_root.is_none() {
            info!("git_mode=none, skipping merge for task {}", task_id);
            return Ok(());
        }
        let branch = TaskId::new(task_id).branch_name();

        // Check if branch exists
        if self.git(&["rev-parse", "--verify", &branch]).is_err() {
            info!("branch {branch} not found, skipping merge");
            return Ok(());
        }

        // Ensure the target branch exists locally.  When a project's
        // default_branch is changed (e.g. from "main" to "dev") after the repo
        // was already cloned, the new branch won't exist yet.  Create it from
        // the current HEAD so the first merge can succeed.
        if self.git(&["rev-parse", "--verify", target_branch]).is_err() {
            info!("target branch {target_branch} does not exist locally, creating from HEAD");
            self.git(&["branch", target_branch])
                .with_context(|| format!("create missing target branch {target_branch}"))?;
        }

        // Check current branch
        let current = self.git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if current.trim() != target_branch {
            self.git(&["checkout", target_branch])
                .context("checkout target branch for merge")?;
        }

        // Pull with rebase to incorporate other orchestras' work before merging.
        // Only contact the remote when auto_push is enabled.
        if self.auto_push.load(Ordering::Relaxed)
            && let Err(e) = self.run_git(
                &["pull", "--rebase", "origin", target_branch],
                GIT_NET_TIMEOUT_SECS,
            )
        {
            warn!("pull --rebase before merge failed: {e}");
            self.git(&["rebase", "--abort"]).ok();
            *self.last_fetch.lock().unwrap() = None;
        }

        // Save pre-merge HEAD so we can roll back if push fails.
        let pre_merge_head = self
            .git_output(&["rev-parse", "HEAD"])
            .context("save pre-merge HEAD")?
            .trim()
            .to_string();

        // Attempt merge
        match self.git(&["merge", "--no-edit", &branch]) {
            Ok(_) => {
                info!("merged {branch} -> {target_branch}");
                if self.auto_push.load(Ordering::Relaxed) {
                    // Delete remote branch if it was pushed
                    if self
                        .git(&["rev-parse", "--verify", &format!("origin/{branch}")])
                        .is_ok()
                    {
                        if let Err(e) = self.git(&["push", "origin", "--delete", &branch]) {
                            warn!("failed to delete remote branch {branch}: {e}");
                        } else {
                            info!("deleted remote branch {branch}");
                        }
                    }
                    // Push merged target to origin, retrying on concurrent changes.
                    // If push fails, roll back the local merge to avoid leaving the
                    // target branch in a diverged state (the task branch is preserved
                    // so the merge can be retried later).
                    if let Err(e) = self.push_branch_with_retry(target_branch) {
                        error!(
                            "push failed after merge, resetting {target_branch} to pre-merge state: {e}"
                        );
                        self.git(&["reset", "--hard", &pre_merge_head]).ok();
                        *self.last_fetch.lock().unwrap() = None;
                        bail!(
                            "push {target_branch} failed after merge of {branch}: {e}; \
                             local branch reset to pre-merge state"
                        );
                    }
                    // Push succeeded — safe to delete the local task branch now
                    self.git(&["branch", "-d", &branch]).ok();
                    *self.last_fetch.lock().unwrap() = None;
                } else {
                    info!("auto_push disabled, skipping push to origin");
                    self.git(&["branch", "-d", &branch]).ok();
                }
                Ok(())
            }
            Err(_) => {
                error!("merge conflict on {branch} -> {target_branch}, aborting");
                self.git(&["merge", "--abort"]).ok();
                bail!("merge conflict on {branch}")
            }
        }
    }

    /// Push a task branch to origin without merging (for `branch_only` strategy).
    pub fn push_task_branch(&self, task_id: &str) -> Result<()> {
        if self.git_root.is_none() {
            info!("git_mode=none, skipping push for task {}", task_id);
            return Ok(());
        }
        let branch = TaskId::new(task_id).branch_name();

        if self.git(&["rev-parse", "--verify", &branch]).is_err() {
            info!("branch {branch} not found, skipping push");
            return Ok(());
        }

        self.run_git(&["push", "-u", "origin", &branch], GIT_NET_TIMEOUT_SECS)
            .with_context(|| format!("push task branch {branch} to origin"))?;
        info!("pushed task branch {branch} to origin");
        *self.last_fetch.lock().unwrap() = None;
        Ok(())
    }

    /// Revert a task's changes from the default branch.
    ///
    /// Finds the merge commit (or individual commits) introduced by the task
    /// and creates revert commit(s) on the default branch. If auto_push is
    /// enabled, pushes the revert to origin.
    pub fn revert_task(&self, task_id: &str) -> Result<String> {
        if self.git_root.is_none() {
            bail!("git operations disabled for this project");
        }
        let tid = TaskId::new(task_id);
        let branch = tid.branch_name();
        let db = &self.default_branch;

        // Ensure we're on the default branch
        let current = self.git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if current.trim() != db {
            self.git(&["checkout", db])
                .context("checkout default branch for revert")?;
        }

        // Strategy 1: Look for a merge commit that merged this task's branch.
        // The merge commit message is "Merge branch 'agent/task-xxx' into ..."
        let merge_log = self.git_output(&[
            "log",
            "--merges",
            "--format=%H",
            "--grep",
            &format!("Merge branch '{branch}'"),
            db,
        ])?;

        if let Some(merge_hash) = merge_log
            .lines()
            .next()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
        {
            // Found a merge commit — revert it
            self.git(&["revert", "-m", "1", "--no-edit", merge_hash])
                .with_context(|| format!("revert merge commit {merge_hash}"))?;

            let short_hash = &merge_hash[..8.min(merge_hash.len())];
            info!("reverted merge commit {short_hash} for task {tid}");

            // Push if auto_push is enabled
            if self.auto_push.load(Ordering::Relaxed) {
                self.push_branch_with_retry(db)?;
                *self.last_fetch.lock().unwrap() = None;
            }

            return Ok(format!("Reverted merge commit {short_hash}"));
        }

        // Strategy 2: No merge commit found (fast-forward merge).
        // Search for individual commits by the agent(<short_id>) suffix.
        let log_output = self.git_output(&[
            "log",
            "--format=%H",
            "--grep",
            &format!("agent({})", tid.short()),
            db,
        ])?;

        let commits: Vec<&str> = log_output
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        if commits.is_empty() {
            bail!(
                "No commits found for task {tid} on {db}. The task may not have been merged yet."
            );
        }

        // Revert commits in order (git log returns newest first, which is correct
        // for revert — we undo the newest commit first).
        for hash in &commits {
            self.git(&["revert", "--no-edit", hash])
                .with_context(|| format!("revert commit {hash}"))?;
        }

        info!("reverted {} commit(s) for task {tid}", commits.len());

        // Push if auto_push is enabled
        if self.auto_push.load(Ordering::Relaxed) {
            self.push_branch_with_retry(db)?;
            *self.last_fetch.lock().unwrap() = None;
        }

        Ok(format!(
            "Reverted {} commit(s) for task {}",
            commits.len(),
            tid.short()
        ))
    }

    /// Push a branch to origin, retrying with pull --rebase if push fails
    /// due to concurrent remote changes.
    pub(super) fn push_branch_with_retry(&self, branch: &str) -> Result<()> {
        for attempt in 1..=Self::PUSH_RETRIES {
            match self.run_git(&["push", "origin", branch], GIT_NET_TIMEOUT_SECS) {
                Ok(_) => {
                    info!("pushed {branch} to origin (attempt {attempt})");
                    return Ok(());
                }
                Err(e) if attempt < Self::PUSH_RETRIES => {
                    warn!(
                        "push {branch} failed (attempt {attempt}/{}): {e}, retrying with pull --rebase",
                        Self::PUSH_RETRIES
                    );
                    if let Err(rebase_err) = self.run_git(
                        &["pull", "--rebase", "origin", branch],
                        GIT_NET_TIMEOUT_SECS,
                    ) {
                        error!("pull --rebase failed during push retry: {rebase_err}");
                        self.git(&["rebase", "--abort"]).ok();
                        *self.last_fetch.lock().unwrap() = None;
                        bail!("push {branch} failed and pull --rebase failed: {rebase_err}");
                    }
                }
                Err(e) => {
                    error!(
                        "push {branch} failed after {} attempts: {e}",
                        Self::PUSH_RETRIES
                    );
                    bail!(
                        "push {branch} to origin failed after {} attempts: {e}",
                        Self::PUSH_RETRIES
                    );
                }
            }
        }
        unreachable!()
    }

    /// Delete local and remote `agent/task-*` branches that are fully merged into
    /// the default branch.
    ///
    /// This catches branches that were merged but whose deletion failed (e.g. network
    /// error on `git push origin --delete`) or branches left behind from previous runs.
    /// Safe: only deletes branches that `git branch --merged <default>` confirms are merged.
    pub fn cleanup_merged_branches(&self) {
        if self.git_root.is_none() {
            return;
        }

        // ── Local merged branches ──
        let local_merged = match self.git_output(&["branch", "--merged", &self.default_branch]) {
            Ok(output) => output,
            Err(e) => {
                warn!("cleanup_merged_branches: failed to list merged branches: {e}");
                return;
            }
        };

        let mut local_deleted = 0u32;
        for line in local_merged.lines() {
            let branch = line.trim().trim_start_matches("* ");
            if branch.starts_with("agent/task-") {
                match self.git(&["branch", "-d", branch]) {
                    Ok(_) => {
                        info!("cleanup: deleted merged local branch {branch}");
                        local_deleted += 1;
                    }
                    Err(e) => {
                        warn!("cleanup: failed to delete local branch {branch}: {e}");
                    }
                }
            }
        }

        // ── Remote merged branches ──
        // Only clean remote branches if auto_push is enabled (we manage the remote)
        if !self.auto_push.load(Ordering::Relaxed) {
            if local_deleted > 0 {
                info!("cleanup_merged_branches: deleted {local_deleted} merged local branch(es)");
            }
            return;
        }

        // Fetch to ensure we have current remote state
        self.fetch_if_stale();

        let remote_merged = match self.git_output(&[
            "branch",
            "-r",
            "--merged",
            &self.default_branch,
        ]) {
            Ok(output) => output,
            Err(e) => {
                warn!("cleanup_merged_branches: failed to list remote merged branches: {e}");
                if local_deleted > 0 {
                    info!(
                        "cleanup_merged_branches: deleted {local_deleted} merged local branch(es)"
                    );
                }
                return;
            }
        };

        let mut remote_deleted = 0u32;
        for line in remote_merged.lines() {
            let ref_name = line.trim();
            // Match origin/agent/task-* refs
            if let Some(branch) = ref_name.strip_prefix("origin/")
                && branch.starts_with("agent/task-")
            {
                match self.run_git(
                    &["push", "origin", "--delete", branch],
                    GIT_NET_TIMEOUT_SECS,
                ) {
                    Ok(_) => {
                        info!("cleanup: deleted merged remote branch {branch}");
                        remote_deleted += 1;
                    }
                    Err(e) => {
                        warn!("cleanup: failed to delete remote branch {branch}: {e}");
                    }
                }
            }
        }

        if local_deleted > 0 || remote_deleted > 0 {
            info!(
                "cleanup_merged_branches: deleted {local_deleted} local + {remote_deleted} remote merged branch(es)"
            );
        }
    }

    /// Push an agent/* branch to a remote.
    pub fn push_branch(&self, branch: &str, remote: &str) -> Result<String> {
        if self.git_root.is_none() {
            bail!("git_mode=none: push operations not supported");
        }
        if !branch.starts_with("agent/") {
            bail!("only agent/* branches can be pushed");
        }

        // Verify branch exists
        self.git(&["rev-parse", "--verify", branch])
            .with_context(|| format!("branch '{branch}' does not exist"))?;

        let output = self.run_git(&["push", "-u", remote, branch], GIT_NET_TIMEOUT_SECS)?;

        // Invalidate fetch cache so the next status check fetches fresh remote state
        *self.last_fetch.lock().unwrap() = None;

        Ok(format!("Pushed {branch} to {remote}: {}", output.trim()))
    }

    /// Push the default branch to origin.
    pub fn push_main(&self) -> Result<String> {
        if self.git_root.is_none() {
            bail!("git_mode=none: push operations not supported");
        }
        let db = &self.default_branch;
        self.git(&["rev-parse", "--verify", db])
            .with_context(|| format!("{db} branch does not exist"))?;

        self.run_git(&["push", "origin", db], GIT_NET_TIMEOUT_SECS)
            .with_context(|| format!("push {db} to origin"))?;

        // Invalidate fetch cache so the next status check fetches fresh remote state
        *self.last_fetch.lock().unwrap() = None;

        Ok(format!("Pushed {db} to origin"))
    }

    /// Resolve diverged default branch by rebasing onto origin, then push.
    ///
    /// Used when the local default branch has commits but origin is also ahead (diverged).
    /// Strategy: `git pull --rebase origin <default>` then `git push origin <default>`.
    pub fn resolve_and_push_main(&self) -> Result<String> {
        if self.git_root.is_none() {
            bail!("git_mode=none: push operations not supported");
        }
        let db = &self.default_branch;
        self.git(&["rev-parse", "--verify", db])
            .with_context(|| format!("{db} branch does not exist"))?;

        // Ensure we are on the default branch
        let current = self
            .git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default();
        if current.trim() != db {
            self.git(&["checkout", db])
                .with_context(|| format!("checkout {db} before resolve"))?;
        }

        // Rebase local commits on top of remote
        if let Err(e) = self.run_git(&["pull", "--rebase", "origin", db], GIT_NET_TIMEOUT_SECS) {
            warn!("pull --rebase failed in resolve_and_push_main: {e}");
            // Abort any in-progress rebase so the tree is clean
            self.git(&["rebase", "--abort"]).ok();
            // Invalidate fetch cache so the next attempt fetches fresh remote state
            *self.last_fetch.lock().unwrap() = None;
            bail!("pull --rebase origin {db} failed: {e}");
        }

        // Push the rebased commits
        self.run_git(&["push", "origin", db], GIT_NET_TIMEOUT_SECS)
            .with_context(|| format!("push origin {db} after rebase"))?;

        // Invalidate fetch cache so the next status check fetches fresh remote state
        *self.last_fetch.lock().unwrap() = None;

        Ok(format!("Resolved conflict and pushed {db} to origin"))
    }

    /// Rebase a task branch onto the default branch to resolve divergence.
    ///
    /// This updates the task branch so it is based on the latest default branch,
    /// resolving the "stranded branch" problem where the branch can't merge
    /// due to conflicts with newer commits on default.
    pub fn resolve_task_branch(&self, task_id: &str) -> Result<String> {
        if self.git_root.is_none() {
            bail!("git_mode=none: resolve operations not supported");
        }

        let branch = TaskId::new(task_id).branch_name();

        // Verify the task branch exists
        self.git(&["rev-parse", "--verify", &branch])
            .with_context(|| format!("branch '{branch}' does not exist"))?;

        // Save current branch to restore later
        let original_branch = self
            .git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_string();

        // Checkout the task branch
        self.git(&["checkout", &branch])
            .context("checkout task branch for rebase")?;

        // Rebase onto default branch
        match self.git(&["rebase", &self.default_branch]) {
            Ok(_) => {
                info!("rebased {branch} onto {}", self.default_branch);
                // Force-push the rebased branch if it was previously pushed
                let remote_ref = format!("origin/{branch}");
                if self.git(&["rev-parse", "--verify", &remote_ref]).is_ok() {
                    if let Err(e) = self.run_git(
                        &["push", "--force-with-lease", "origin", &branch],
                        GIT_NET_TIMEOUT_SECS,
                    ) {
                        warn!("failed to force-push rebased branch {branch}: {e}");
                        // Not fatal — the local rebase succeeded
                    } else {
                        info!("force-pushed rebased branch {branch}");
                    }
                }
                // Return to original branch
                self.git(&["checkout", &original_branch]).ok();
                Ok(format!("Rebased {branch} onto {}", self.default_branch))
            }
            Err(e) => {
                error!(
                    "rebase of {branch} onto {} failed: {e}",
                    self.default_branch
                );
                self.git(&["rebase", "--abort"]).ok();
                self.git(&["checkout", &original_branch]).ok();
                bail!("rebase failed on {branch} — manual conflict resolution required")
            }
        }
    }
}
