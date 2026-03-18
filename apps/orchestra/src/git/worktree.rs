use crate::task_id::TaskId;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use super::{GIT_NET_TIMEOUT_SECS, WorktreeManager, git_in};

impl WorktreeManager {
    pub(super) fn worktree_path(&self, task_id: &str) -> PathBuf {
        self.worktree_dir
            .join(TaskId::new(task_id).worktree_dir_name())
    }

    /// Ensure a branch exists locally. If it doesn't, create it from the default branch.
    /// Used by the feature_branch strategy to ensure the work branch exists before
    /// creating task worktrees.
    pub fn ensure_branch(&self, branch: &str) -> Result<()> {
        if self.git_root.is_none() {
            return Ok(());
        }

        // Check if branch already exists
        if self.git(&["rev-parse", "--verify", branch]).is_ok() {
            return Ok(());
        }

        // Check if it exists on remote
        self.fetch_if_stale();
        let remote_ref = format!("origin/{branch}");
        if self.git(&["rev-parse", "--verify", &remote_ref]).is_ok() {
            info!("creating local branch {branch} from {remote_ref}");
            self.git(&["branch", branch, &remote_ref])
                .with_context(|| format!("create branch {branch} from {remote_ref}"))?;
        } else {
            info!(
                "creating branch {branch} from {} (not on remote)",
                self.default_branch
            );
            self.git(&["branch", branch, &self.default_branch])
                .with_context(|| format!("create branch {branch} from {}", self.default_branch))?;
        }
        Ok(())
    }

    /// Create a worktree for a task, reusing an existing one if present.
    pub fn create_worktree(&self, task_id: &str) -> Result<PathBuf> {
        let path = self.worktree_path(task_id);

        // git_mode=none: create a plain directory, no git worktree
        if self.git_root.is_none() {
            std::fs::create_dir_all(&path).context("create task directory")?;
            return Ok(path);
        }

        let tid = TaskId::new(task_id);
        let branch = tid.branch_name();

        std::fs::create_dir_all(&self.worktree_dir).context("create worktree directory")?;

        // Reuse existing worktree (multi-step pipeline)
        if path.exists() {
            info!("reusing worktree for {branch} at {}", path.display());
            return Ok(path);
        }

        // Fetch and fast-forward the default branch so new worktrees
        // start from the latest remote state.
        self.fetch_if_stale();
        let db = &self.default_branch;
        if let Err(e) = self.git(&["checkout", db]) {
            // Default branch doesn't exist locally — create it from HEAD
            warn!("failed to checkout {db} for fast-forward: {e}");
            if self.git(&["rev-parse", "--verify", db]).is_err() {
                info!("default branch {db} does not exist, creating from HEAD");
                if let Err(e2) = self.git(&["checkout", "-b", db]) {
                    warn!("failed to create default branch {db}: {e2}");
                }
            }
        } else if let Err(e) = self.run_git(
            &["merge", "--ff-only", &format!("origin/{db}")],
            GIT_NET_TIMEOUT_SECS,
        ) {
            // Not fatal — we'll branch from whatever local state we have.
            warn!("fast-forward {db} from origin failed: {e} — branching from local state");
        }

        // Verify the default branch is a valid ref before attempting worktree creation.
        // This catches empty repos where HEAD is unborn and `checkout -b` creates a
        // branch that isn't backed by any commit.
        if self.git(&["rev-parse", "--verify", db]).is_err() {
            bail!(
                "default branch '{db}' is not a valid reference — \
                 the repository may have no commits. \
                 Push at least one commit before assigning tasks."
            );
        }

        // Clean stale state (safe delete — keeps unmerged branches)
        self.git(&["worktree", "prune"]).ok();
        if self.git(&["branch", "-d", &branch]).is_err() {
            // Branch exists with unmerged commits — reattach worktree to preserve work.
            // This happens after orchestra restart: cleanup_all removes worktree dirs
            // but keeps unmerged branches. Rather than bailing, create a new worktree
            // from the existing branch so the agent can continue where it left off.
            if self.git(&["rev-parse", "--verify", &branch]).is_ok() {
                info!(
                    "reattaching worktree for existing branch {branch} at {}",
                    path.display()
                );
                self.git(&["worktree", "add", path.to_str().unwrap(), &branch])
                    .with_context(|| {
                        format!(
                            "reattach worktree for branch {branch} at {}",
                            path.display()
                        )
                    })?;
                return Ok(path);
            }
        }

        // Create new worktree from the default branch
        self.git(&["worktree", "add", "-b", &branch, path.to_str().unwrap(), db])
            .with_context(|| format!("create worktree at {}", path.display()))?;

        Ok(path)
    }

    /// Remove a worktree and its branch (safe: only deletes fully-merged branches).
    pub fn remove_worktree(&self, task_id: &str) {
        if self.git_root.is_none() {
            let path = self.worktree_path(task_id);
            std::fs::remove_dir_all(&path).ok();
            return;
        }
        let path = self.worktree_path(task_id);
        let branch = TaskId::new(task_id).branch_name();
        if path.exists() {
            self.git(&["worktree", "remove", "--force", path.to_str().unwrap()])
                .ok();
        }
        // Safe delete — refuses if branch has unmerged commits
        if self.git(&["branch", "-d", &branch]).is_err() {
            warn!("kept branch {branch} (has unmerged commits)");
        }
    }

    /// Remove worktree by path directly (safe: only deletes fully-merged branches).
    pub fn remove_worktree_path(&self, path: &Path) {
        if self.git_root.is_none() {
            std::fs::remove_dir_all(path).ok();
            return;
        }
        if path.exists() {
            // Derive branch name from path (e.g. task-abcdef012345 -> agent/task-abcdef012345)
            let branch = path
                .file_name()
                .map(|n| format!("agent/{}", n.to_string_lossy()))
                .unwrap_or_default();
            self.git(&["worktree", "remove", "--force", path.to_str().unwrap()])
                .ok();
            if !branch.is_empty() && self.git(&["branch", "-d", &branch]).is_err() {
                warn!("kept branch {branch} (has unmerged commits)");
            }
        }
    }

    /// Commit any uncommitted changes in a worktree.
    pub fn commit_changes(&self, worktree_path: &Path, task_id: &str) -> Result<bool> {
        if self.git_root.is_none() {
            return Ok(false);
        }
        let tid = TaskId::new(task_id);

        // Check for changes
        let status = git_in(worktree_path, &["status", "--porcelain"])?;
        if status.trim().is_empty() {
            return Ok(false);
        }

        git_in(worktree_path, &["add", "-A"])?;
        git_in(
            worktree_path,
            &[
                "commit",
                "-m",
                &format!("task completed agent({})", tid.short()),
            ],
        )?;
        Ok(true)
    }
}
