use crate::task_id::TaskId;
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Shared fetch timestamp — one per repo root.
type FetchTimestamp = Arc<Mutex<Option<Instant>>>;

/// Global registry of per-repo fetch timestamps.
///
/// Each repo root maps to a shared `FetchTimestamp`. All `WorktreeManager`
/// instances for the same repo share the same timestamp, so the 60-second
/// cooldown in `fetch_if_stale()` actually works across per-request
/// instantiations (e.g. WebSocket git requests).
static FETCH_REGISTRY: LazyLock<Mutex<HashMap<PathBuf, FetchTimestamp>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Cooldown between `git fetch` calls (seconds).
const FETCH_COOLDOWN_SECS: u64 = 60;

/// Timeout for regular git commands (rev-parse, log, for-each-ref, etc.).
const GIT_CMD_TIMEOUT_SECS: u64 = 30;

/// Timeout for git network operations (fetch, push).
const GIT_NET_TIMEOUT_SECS: u64 = 60;

/// Manages git worktrees for task isolation.
pub struct WorktreeManager {
    /// Git repository root. None when git_mode=none (all git ops disabled).
    git_root: Option<PathBuf>,
    worktree_dir: PathBuf,
    /// Last time `git fetch` was run — used to throttle network calls.
    /// Shared across all WorktreeManager instances for the same repo root
    /// via `FETCH_REGISTRY`, so per-request instantiation doesn't bypass
    /// the cooldown.
    last_fetch: Arc<Mutex<Option<Instant>>>,
    /// When true, merge operations push to origin after merging and pull
    /// remote changes before merge. Default false (opt-in).
    auto_push: AtomicBool,
    /// The project's default branch (e.g. "main", "develop"). All merge/push
    /// operations target this branch instead of hardcoding "main".
    default_branch: String,
}

impl WorktreeManager {
    pub fn new(repo_root: &Path) -> Self {
        Self::with_branch(repo_root, "main")
    }

    /// Create a WorktreeManager targeting a specific default branch.
    ///
    /// The fetch-cooldown timestamp is shared across all `WorktreeManager`
    /// instances for the same `repo_root`, so per-request instantiation
    /// doesn't bypass the 60-second fetch throttle.
    pub fn with_branch(repo_root: &Path, default_branch: &str) -> Self {
        let worktree_dir = repo_root.join(".claude/worktrees");
        let last_fetch = FETCH_REGISTRY
            .lock()
            .unwrap()
            .entry(repo_root.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone();
        Self {
            git_root: Some(repo_root.to_path_buf()),
            worktree_dir,
            last_fetch,
            auto_push: AtomicBool::new(false),
            default_branch: default_branch.to_string(),
        }
    }

    /// Create a no-op WorktreeManager for git_mode=none projects.
    /// Task directories are created under `work_dir/worktrees` but no git operations run.
    pub fn disabled(work_dir: &Path) -> Self {
        Self {
            git_root: None,
            worktree_dir: work_dir.join("worktrees"),
            last_fetch: Arc::new(Mutex::new(None)),
            auto_push: AtomicBool::new(false),
            default_branch: "main".to_string(),
        }
    }

    /// Returns the project's default branch name.
    pub fn default_branch(&self) -> &str {
        &self.default_branch
    }

    /// Returns true if git operations are enabled (git_mode != "none").
    pub fn is_git_enabled(&self) -> bool {
        self.git_root.is_some()
    }

    /// Set whether merge_to_main should push to origin after merging.
    pub fn set_auto_push(&self, enabled: bool) {
        self.auto_push.store(enabled, Ordering::Relaxed);
    }

    /// Returns the git repository root path, if git is enabled.
    pub fn git_root(&self) -> Option<&Path> {
        self.git_root.as_deref()
    }

    /// Ensure a branch exists locally. If it doesn't, create it from the default branch.
    /// Used by the feature_branch strategy to ensure the goal branch exists before
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

    fn worktree_path(&self, task_id: &str) -> PathBuf {
        self.worktree_dir
            .join(TaskId::new(task_id).worktree_dir_name())
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

    /// Maximum number of push retries when concurrent remote changes cause push to fail.
    const PUSH_RETRIES: usize = 3;

    /// Merge task branch to main, then push to origin.
    ///
    /// Before merging, pulls with rebase to incorporate other orchestras' work.
    /// After merging, pushes to origin so the merge is visible to all orchestras.
    /// If push fails (e.g. concurrent remote change), retries pull --rebase + push.
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
    /// `feature_branch` targets the goal branch.
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

        // Attempt merge
        match self.git(&["merge", "--no-edit", &branch]) {
            Ok(_) => {
                info!("merged {branch} -> {target_branch}");
                self.git(&["branch", "-d", &branch]).ok();
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
                    // Push merged target to origin, retrying on concurrent changes
                    self.push_branch_with_retry(target_branch)?;
                    *self.last_fetch.lock().unwrap() = None;
                } else {
                    info!("auto_push disabled, skipping push to origin");
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
    fn push_branch_with_retry(&self, branch: &str) -> Result<()> {
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

    /// Clean up all task worktrees.
    pub fn cleanup_all(&self) {
        if self.git_root.is_none() {
            // Clean up plain task directories
            if self.worktree_dir.exists()
                && let Ok(entries) = std::fs::read_dir(&self.worktree_dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        std::fs::remove_dir_all(&path).ok();
                    }
                }
            }
            return;
        }
        if self.worktree_dir.exists()
            && let Ok(entries) = std::fs::read_dir(&self.worktree_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && path
                        .file_name()
                        .map(|n| n.to_string_lossy().starts_with("task-"))
                        .unwrap_or(false)
                {
                    self.remove_worktree_path(&path);
                }
            }
        }
        self.git(&["worktree", "prune"]).ok();
        self.cleanup_merged_branches();
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

    /// Collect changed files between the branch and the default branch, with per-file diffs.
    pub fn collect_changed_files(&self, task_id: &str) -> Result<Vec<ChangedFile>> {
        if self.git_root.is_none() {
            return Ok(Vec::new());
        }
        let tid = TaskId::new(task_id);
        let branch = tid.branch_name();
        let db = &self.default_branch;

        // Check if branch exists
        if self.git(&["rev-parse", "--verify", &branch]).is_err() {
            info!("collect_changed_files {tid}: branch {branch} not found");
            return Ok(Vec::new());
        }

        // Get list of changed files with status (A/M/D)
        let diff_output =
            self.git_output(&["diff", "--name-status", &format!("{db}...{branch}")])?;
        info!(
            "collect_changed_files {tid}: diff {db}...{branch} returned {} lines",
            diff_output.lines().filter(|l| !l.trim().is_empty()).count()
        );

        let mut files = Vec::new();
        for line in diff_output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let status_char = parts.next().unwrap_or("").trim();
            let path = parts.next().unwrap_or("").trim().to_string();
            if path.is_empty() {
                continue;
            }

            let change_type = match status_char.chars().next() {
                Some('A') => "added",
                Some('D') => "deleted",
                _ => "modified",
            }
            .to_string();

            // Get per-file diff
            let diff = self
                .git_output(&["diff", &format!("{db}...{branch}"), "--", &path])
                .ok()
                .filter(|d| !d.trim().is_empty());

            files.push(ChangedFile {
                path,
                change_type,
                diff,
            });
        }
        Ok(files)
    }

    /// Return (insertions, deletions) for the diff between the default branch and the task branch.
    ///
    /// Uses `git diff --shortstat <default>...branch` to count total added/removed lines.
    /// Returns (0, 0) if git is disabled or the branch does not exist.
    /// Used by the orchestra to detect implement steps that delete far more than they add,
    /// which is a strong signal of collateral damage (agent used Write instead of Edit).
    pub fn diff_insertion_deletion_stats(&self, task_id: &str) -> Result<(usize, usize)> {
        if self.git_root.is_none() {
            return Ok((0, 0));
        }
        let branch = TaskId::new(task_id).branch_name();
        if self.git(&["rev-parse", "--verify", &branch]).is_err() {
            return Ok((0, 0));
        }
        let db = &self.default_branch;
        let shortstat = self.git_output(&["diff", "--shortstat", &format!("{db}...{branch}")])?;
        // Format: " 3 files changed, 12 insertions(+), 45 deletions(-)"
        let insertions = shortstat
            .split_whitespace()
            .zip(shortstat.split_whitespace().skip(1))
            .find(|(_, w)| w.starts_with("insertion"))
            .and_then(|(n, _)| n.parse::<usize>().ok())
            .unwrap_or(0);
        let deletions = shortstat
            .split_whitespace()
            .zip(shortstat.split_whitespace().skip(1))
            .find(|(_, w)| w.starts_with("deletion"))
            .and_then(|(n, _)| n.parse::<usize>().ok())
            .unwrap_or(0);
        Ok((insertions, deletions))
    }

    /// Run `git fetch --prune --quiet` at most once per cooldown period.
    ///
    /// The mutex is released before running `git fetch` to avoid blocking
    /// concurrent git operations while a slow fetch is in progress.
    fn fetch_if_stale(&self) {
        let should_fetch = {
            let last = self.last_fetch.lock().unwrap();
            match *last {
                None => true,
                Some(t) => t.elapsed() > Duration::from_secs(FETCH_COOLDOWN_SECS),
            }
        }; // mutex released here — fetch doesn't block other git ops
        if should_fetch {
            match self.run_git(
                &["fetch", "origin", "--prune", "--quiet"],
                GIT_NET_TIMEOUT_SECS,
            ) {
                Ok(_) => {
                    *self.last_fetch.lock().unwrap() = Some(Instant::now());
                }
                Err(e) => {
                    warn!("git fetch origin failed: {e} — remote tracking refs may be stale");
                }
            }
        }
    }

    fn git(&self, args: &[&str]) -> Result<()> {
        self.run_git(args, GIT_CMD_TIMEOUT_SECS)?;
        Ok(())
    }

    /// Run a git command with a timeout. Returns stdout on success.
    fn run_git(&self, args: &[&str], timeout_secs: u64) -> Result<String> {
        let root = self
            .git_root
            .as_ref()
            .expect("run_git called with git disabled");
        run_git_in(root, args, timeout_secs)
    }

    /// List branches matching a prefix with push/ahead/behind info.
    pub fn list_branches(&self, prefix: &str) -> Result<BranchListResult> {
        if self.git_root.is_none() {
            return Ok(BranchListResult {
                current_branch: "none".to_string(),
                branches: vec![],
            });
        }
        let current_branch = self
            .git_output(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_else(|_| "unknown".into())
            .trim()
            .to_string();

        // Fetch remotes (throttled — at most once per cooldown period)
        self.fetch_if_stale();

        let format =
            "%(refname:short)\t%(objectname:short)\t%(upstream:short)\t%(upstream:trackshort)";
        let refs_pattern = format!("refs/heads/{prefix}*");
        let output = self
            .git_output(&["for-each-ref", &format!("--format={format}"), &refs_pattern])
            .unwrap_or_default();

        let mut branches = Vec::new();
        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            let name = parts.first().unwrap_or(&"").to_string();
            let commit = parts.get(1).unwrap_or(&"").to_string();
            let upstream = parts.get(2).unwrap_or(&"").to_string();
            let track = parts.get(3).unwrap_or(&"").to_string();

            let is_pushed = !upstream.is_empty();
            let (ahead, behind) = parse_track_short(&track);

            let task_id_prefix = name.strip_prefix("agent/task-").map(|s| s.to_string());

            branches.push(BranchInfoResult {
                name,
                commit,
                is_pushed,
                ahead_remote: ahead,
                behind_remote: behind,
                task_id_prefix,
            });
        }

        Ok(BranchListResult {
            current_branch,
            branches,
        })
    }

    /// Check status of a task branch.
    pub fn task_branch_status(&self, task_id: &str) -> Result<TaskBranchResult> {
        if self.git_root.is_none() {
            let branch = TaskId::new(task_id).branch_name();
            return Ok(TaskBranchResult {
                branch,
                exists: false,
                is_pushed: false,
                ahead_remote: 0,
                behind_remote: 0,
                last_commit: None,
                last_commit_message: None,
                behind_default: 0,
                has_conflict: false,
            });
        }
        let branch = TaskId::new(task_id).branch_name();

        let exists = self.git(&["rev-parse", "--verify", &branch]).is_ok();

        if !exists {
            return Ok(TaskBranchResult {
                branch,
                exists: false,
                is_pushed: false,
                ahead_remote: 0,
                behind_remote: 0,
                last_commit: None,
                last_commit_message: None,
                behind_default: 0,
                has_conflict: false,
            });
        }

        let remote_ref = format!("origin/{branch}");
        let is_pushed = self.git(&["rev-parse", "--verify", &remote_ref]).is_ok();

        let (ahead, behind) = if is_pushed {
            self.get_ahead_behind(&branch, &remote_ref)
        } else {
            (0, 0)
        };

        let last_commit = self
            .git_output(&["log", "-1", "--format=%h", &branch])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let last_commit_message = self
            .git_output(&["log", "-1", "--format=%s", &branch])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        // Check how far behind the default branch this task branch is.
        let (_ahead_default, behind_default) = self.get_ahead_behind(&branch, &self.default_branch);

        // Check for merge conflicts using git merge-tree (available since Git 2.38).
        // Only check when the branch has diverged from default (behind > 0).
        let has_conflict = if behind_default > 0 {
            self.check_merge_conflict(&branch, &self.default_branch)
        } else {
            false
        };

        Ok(TaskBranchResult {
            branch,
            exists,
            is_pushed,
            ahead_remote: ahead,
            behind_remote: behind,
            last_commit,
            last_commit_message,
            behind_default,
            has_conflict,
        })
    }

    /// Check if merging `branch` into `target` would produce conflicts.
    ///
    /// Uses `git merge-tree --write-tree` (Git 2.38+) for a work-tree-free
    /// conflict check.  Falls back to `false` if the command is unavailable.
    fn check_merge_conflict(&self, branch: &str, target: &str) -> bool {
        // git merge-tree --write-tree <target> <branch>
        // Exit 0 = clean merge, non-zero = conflicts or error.
        // We interpret any non-zero as "has conflict" since that's the safe default.
        let root = match &self.git_root {
            Some(r) => r,
            None => return false,
        };
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["merge-tree", "--write-tree", target, branch])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        match output {
            Ok(status) => !status.success(),
            Err(_) => false, // command not available, assume no conflict
        }
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

    /// Check push status of the default branch relative to origin.
    pub fn main_push_status(&self) -> Result<MainPushStatus> {
        if self.git_root.is_none() {
            return Ok(MainPushStatus {
                ahead: 0,
                behind: 0,
                last_commit: None,
                last_commit_message: None,
            });
        }
        // Fetch remotes (throttled — at most once per cooldown period)
        self.fetch_if_stale();

        let db = &self.default_branch;

        // Check if local branch exists
        if self.git(&["rev-parse", "--verify", db]).is_err() {
            info!("main_push_status: local branch '{db}' does not exist, returning 0/0");
            return Ok(MainPushStatus {
                ahead: 0,
                behind: 0,
                last_commit: None,
                last_commit_message: None,
            });
        }

        let remote_ref = format!("origin/{db}");
        let mut has_remote = self.git(&["rev-parse", "--verify", &remote_ref]).is_ok();

        // If remote ref is missing, try a targeted fetch of just this branch
        // in case the throttled fetch didn't run or failed.
        if !has_remote {
            debug!("main_push_status: {remote_ref} not found, trying targeted fetch of '{db}'");
            if self
                .run_git(&["fetch", "origin", db, "--quiet"], GIT_NET_TIMEOUT_SECS)
                .is_ok()
            {
                has_remote = self.git(&["rev-parse", "--verify", &remote_ref]).is_ok();
            }
        }

        let (ahead, behind) = if has_remote {
            self.get_ahead_behind(db, &remote_ref)
        } else {
            // Remote branch doesn't exist yet — count local commits as "ahead"
            // so the UI shows there's unpushed work. `git push` will create it.
            let ahead = self
                .git_output(&["rev-list", "--count", db])
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(0);
            debug!(
                "main_push_status: {remote_ref} does not exist on remote — {ahead} local commit(s) ahead"
            );
            (ahead, 0)
        };

        let last_commit = self
            .git_output(&["log", "-1", "--format=%h", db])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let last_commit_message = self
            .git_output(&["log", "-1", "--format=%s", db])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Ok(MainPushStatus {
            ahead,
            behind,
            last_commit,
            last_commit_message,
        })
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
    pub fn release(&self, source_branch: Option<&str>, message: Option<&str>) -> Result<String> {
        if self.git_root.is_none() {
            bail!("git_mode=none: release operations not supported");
        }

        let db = &self.default_branch;
        let source = source_branch.unwrap_or("dev");

        // Verify source branch exists
        self.git(&["rev-parse", "--verify", source])
            .with_context(|| format!("source branch '{source}' does not exist"))?;

        // Checkout default branch
        let current = self.git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if current.trim() != db {
            self.git(&["checkout", db])
                .with_context(|| format!("checkout {db} for release"))?;
        }

        // Pull latest if remote is configured
        if self
            .git_output(&["config", "--get", &format!("branch.{db}.remote")])
            .is_ok()
        {
            self.run_git(&["pull", "--rebase", "origin", db], GIT_NET_TIMEOUT_SECS)
                .ok(); // Best-effort; may fail if no remote
        }

        // Check if there's anything to merge
        let diff_check = self.git_output(&["rev-list", "--count", &format!("{db}..{source}")])?;
        let commit_count: i32 = diff_check.trim().parse().unwrap_or(0);
        if commit_count == 0 {
            bail!("nothing to release: {source} has no new commits over {db}");
        }

        // Squash merge
        self.git(&["merge", "--squash", source])
            .with_context(|| format!("squash merge {source} into {db}"))?;

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
                    &format!("{db}..{source}"),
                ])
                .unwrap_or_default();
            format!("release: squash merge {source} into {db}\n\n{log}")
        };

        self.git(&["commit", "-m", &commit_msg])
            .with_context(|| "commit squash merge")?;

        // Generate date-based tag: v{YYYYMMDD}-{NN}
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Convert unix timestamp to YYYYMMDD
        let days = now / 86400;
        let (y, m, d) = unix_days_to_ymd(days as i64);
        let today = format!("{y:04}{m:02}{d:02}");
        let tag_prefix = format!("v{today}-");
        let existing_tags = self
            .git_output(&["tag", "-l", &format!("{tag_prefix}*")])
            .unwrap_or_default();
        let next_seq = existing_tags
            .lines()
            .filter_map(|t| t.strip_prefix(&tag_prefix)?.parse::<i32>().ok())
            .max()
            .map(|n| n + 1)
            .unwrap_or(1);
        let tag = format!("{tag_prefix}{:02}", next_seq);

        self.git(&["tag", &tag])
            .with_context(|| format!("create tag {tag}"))?;

        // Push to all remotes
        let remotes_output = self.git_output(&["remote"]).unwrap_or_default();
        let remotes: Vec<&str> = remotes_output.lines().filter(|r| !r.is_empty()).collect();

        let mut push_results = Vec::new();
        for remote in &remotes {
            // Push default branch
            match self.run_git(&["push", remote, db], GIT_NET_TIMEOUT_SECS) {
                Ok(_) => push_results.push(format!("pushed {db} to {remote}")),
                Err(e) => push_results.push(format!("failed to push {db} to {remote}: {e}")),
            }
            // Push tag
            match self.run_git(&["push", remote, &tag], GIT_NET_TIMEOUT_SECS) {
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

    fn get_ahead_behind(&self, local: &str, remote: &str) -> (i32, i32) {
        let range = format!("{local}...{remote}");
        match self.git_output(&["rev-list", "--left-right", "--count", &range]) {
            Ok(output) => {
                let parts: Vec<&str> = output.split_whitespace().collect();
                let ahead = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                let behind = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                (ahead, behind)
            }
            Err(_) => (0, 0),
        }
    }

    fn git_output(&self, args: &[&str]) -> Result<String> {
        self.run_git(args, GIT_CMD_TIMEOUT_SECS)
    }
}

/// Convert days since Unix epoch to (year, month, day).
fn unix_days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Civil calendar algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

// ── Changed file info (for posting to API) ──

#[derive(Debug, serde::Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub change_type: String,
    pub diff: Option<String>,
}

// ── Result types for git responses ──

#[derive(Debug, serde::Serialize)]
pub struct BranchInfoResult {
    pub name: String,
    pub commit: String,
    pub is_pushed: bool,
    pub ahead_remote: i32,
    pub behind_remote: i32,
    pub task_id_prefix: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct BranchListResult {
    pub current_branch: String,
    pub branches: Vec<BranchInfoResult>,
}

#[derive(Debug, serde::Serialize)]
pub struct TaskBranchResult {
    pub branch: String,
    pub exists: bool,
    pub is_pushed: bool,
    pub ahead_remote: i32,
    pub behind_remote: i32,
    pub last_commit: Option<String>,
    pub last_commit_message: Option<String>,
    /// How many commits behind the default branch this task branch is.
    pub behind_default: i32,
    /// True if merging this branch into default would produce conflicts.
    pub has_conflict: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct MainPushStatus {
    pub ahead: i32,
    pub behind: i32,
    pub last_commit: Option<String>,
    pub last_commit_message: Option<String>,
}

fn parse_track_short(track: &str) -> (i32, i32) {
    match track.trim() {
        ">" => (1, 0),
        "<" => (0, 1),
        "<>" => (1, 1),
        "=" => (0, 0),
        _ => (0, 0),
    }
}

/// Run a git command in a specific directory (non-method version).
fn git_in(dir: &Path, args: &[&str]) -> Result<String> {
    run_git_in(dir, args, GIT_CMD_TIMEOUT_SECS)
}

/// Run a git command in `dir` with a timeout. Returns stdout on success.
///
/// This is the shared subprocess loop used by both `WorktreeManager::run_git`
/// (which passes `self.git_root`) and the free function `git_in` (which passes
/// an explicit directory).
fn run_git_in(dir: &Path, args: &[&str], timeout_secs: u64) -> Result<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn git")?;

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait().context("wait for git")? {
            Some(status) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(ref mut out) = child.stdout {
                    out.read_to_string(&mut stdout).ok();
                }
                if let Some(ref mut err) = child.stderr {
                    err.read_to_string(&mut stderr).ok();
                }
                if !status.success() {
                    bail!("git {}: {stderr}", args.join(" "));
                }
                return Ok(stdout);
            }
            None if Instant::now() >= deadline => {
                warn!(
                    "git {} timed out after {timeout_secs}s, killing process",
                    args.join(" ")
                );
                child.kill().ok();
                child.wait().ok();
                bail!("git {} timed out after {timeout_secs}s", args.join(" "));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    /// Helper: run git in a directory, panicking on failure.
    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run git {}: {e}", args.join(" ")));
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!("git {} failed: {stderr}", args.join(" "));
        }
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Create a bare "origin" repo with an initial commit on main,
    /// and a local clone of it. Returns (origin_dir, local_dir, tempdir_handle).
    fn setup_repos() -> (PathBuf, PathBuf, tempfile::TempDir) {
        let tmp = tempdir().unwrap();
        let origin = tmp.path().join("origin.git");
        let local = tmp.path().join("local");

        // Create bare origin
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "--bare"]);
        // Set default branch to main
        git(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);

        // Create a temporary repo to push an initial commit
        let setup = tmp.path().join("setup");
        git(tmp.path(), &["clone", origin.to_str().unwrap(), "setup"]);
        git(&setup, &["checkout", "-b", "main"]);
        std::fs::write(setup.join("README.md"), "initial\n").unwrap();
        git(&setup, &["add", "README.md"]);
        git(
            &setup,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "init",
            ],
        );
        git(&setup, &["push", "origin", "main"]);

        // Clone origin -> local
        git(tmp.path(), &["clone", origin.to_str().unwrap(), "local"]);
        git(&local, &["checkout", "main"]);
        // Set identity so merge commits work in CI (no global git config)
        git(&local, &["config", "user.name", "Test"]);
        git(&local, &["config", "user.email", "t@t"]);

        (origin, local, tmp)
    }

    /// Create a WorktreeManager pointing at the local clone.
    fn make_manager(local: &Path) -> WorktreeManager {
        WorktreeManager::new(local)
    }

    /// Create a task branch in the local repo with a commit.
    fn create_task_branch(local: &Path, task_id: &str) {
        let tid = TaskId::new(task_id);
        let branch = tid.branch_name();
        git(local, &["checkout", "-b", &branch, "main"]);
        let file = format!("{}.txt", tid.short());
        std::fs::write(local.join(&file), "task work\n").unwrap();
        git(local, &["add", &file]);
        let msg = format!("feat: task work agent({})", tid.short());
        git(
            local,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                &msg,
            ],
        );
        git(local, &["checkout", "main"]);
    }

    // ── Test 1: Happy path ──────────────────────────────────────────

    #[test]
    fn merge_to_main_happy_path() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);
        mgr.set_auto_push(true);
        let task_id = "aaaa-bbbb-cccc-dddd-eeee-ffff";

        create_task_branch(&local, task_id);

        // Merge should succeed
        mgr.merge_to_main(task_id).unwrap();

        // Branch should be deleted after merge
        let branch = TaskId::new(task_id).branch_name();
        let branch_exists = Command::new("git")
            .arg("-C")
            .arg(&local)
            .args(["rev-parse", "--verify", &branch])
            .output()
            .unwrap()
            .status
            .success();
        assert!(!branch_exists, "branch should be deleted after merge");

        // The task file should be on main
        let log = git(&local, &["log", "--oneline", "main"]);
        assert!(log.contains("task work"), "merge commit should be on main");

        // Origin should have the merge (push succeeded)
        let origin_log = git(&_origin, &["log", "--oneline", "main"]);
        assert!(
            origin_log.contains("task work"),
            "origin main should have the merged commit"
        );
    }

    // ── Test 2: Push conflict with retry ────────────────────────────

    #[test]
    fn merge_to_main_push_conflict_retried() {
        let (origin, local, tmp) = setup_repos();
        let mgr = make_manager(&local);
        mgr.set_auto_push(true);
        let task_id = "1111-2222-3333-4444-5555-6666";

        create_task_branch(&local, task_id);

        // Create a concurrent push: clone origin to a second repo, commit+push
        let rival = tmp.path().join("rival");
        git(tmp.path(), &["clone", origin.to_str().unwrap(), "rival"]);
        git(&rival, &["checkout", "main"]);
        std::fs::write(rival.join("rival.txt"), "rival work\n").unwrap();
        git(&rival, &["add", "rival.txt"]);
        git(
            &rival,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "rival",
            ],
        );
        git(&rival, &["push", "origin", "main"]);

        // Now local is behind origin. merge_to_main should pull --rebase first
        // and the push should succeed (possibly after retry).
        mgr.merge_to_main(task_id).unwrap();

        // Origin should have both the rival and task commits
        let origin_log = git(&origin, &["log", "--oneline", "main"]);
        assert!(
            origin_log.contains("rival"),
            "origin should have rival commit"
        );
        assert!(
            origin_log.contains("task work"),
            "origin should have task commit"
        );
    }

    // ── Test 3: last_fetch is invalidated after successful merge+push ──

    #[test]
    fn merge_to_main_invalidates_last_fetch() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);
        mgr.set_auto_push(true);
        let task_id = "dead-beef-1234-5678-9abc-def0";

        // Set last_fetch to a recent timestamp
        *mgr.last_fetch.lock().unwrap() = Some(Instant::now());

        create_task_branch(&local, task_id);

        mgr.merge_to_main(task_id).unwrap();

        // last_fetch should have been reset to None
        let last = mgr.last_fetch.lock().unwrap();
        assert!(
            last.is_none(),
            "last_fetch should be invalidated (None) after successful merge+push"
        );
    }

    // ── Test 4: Rebase-fail fallthrough — merge still proceeds ─────

    #[test]
    fn merge_to_main_rebase_fail_falls_through_to_merge() {
        let tmp = tempdir().unwrap();
        let local = tmp.path().join("local");

        // Create a local repo with NO origin remote so pull --rebase fails
        std::fs::create_dir_all(&local).unwrap();
        git(&local, &["init", "-b", "main"]);
        git(&local, &["config", "user.name", "Test"]);
        git(&local, &["config", "user.email", "t@t"]);

        // Initial commit on main
        std::fs::write(local.join("README.md"), "initial\n").unwrap();
        git(&local, &["add", "README.md"]);
        git(
            &local,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "init",
            ],
        );

        let mgr = make_manager(&local);
        mgr.set_auto_push(true);
        let task_id = "ffff-eeee-dddd-cccc-bbbb-aaaa";

        create_task_branch(&local, task_id);

        // merge_to_main with auto_push=true: pull --rebase will fail (no origin), but
        // merge should proceed. push_main_with_retry will also fail, so the function
        // returns Err overall.
        let result = mgr.merge_to_main(task_id);
        assert!(
            result.is_err(),
            "should return error because push to origin fails"
        );

        // The merge itself should have succeeded — task commit is on local main
        let log = git(&local, &["log", "--oneline", "main"]);
        assert!(
            log.contains("task work"),
            "merge commit should be on local main despite push failure, got: {log}"
        );

        // The task branch should be deleted (merge was successful before push failed)
        let branch = TaskId::new(task_id).branch_name();
        let branch_exists = Command::new("git")
            .arg("-C")
            .arg(&local)
            .args(["rev-parse", "--verify", &branch])
            .output()
            .unwrap()
            .status
            .success();
        assert!(
            !branch_exists,
            "branch should be deleted after successful merge"
        );
    }

    // ── Test 5: resolve_and_push_main aborts rebase on conflict ──────

    #[test]
    fn resolve_and_push_main_aborts_rebase_on_conflict() {
        let (origin, local, tmp) = setup_repos();
        let mgr = make_manager(&local);

        // Create a conflicting commit on origin via a rival clone
        let rival = tmp.path().join("rival");
        git(tmp.path(), &["clone", origin.to_str().unwrap(), "rival"]);
        git(&rival, &["checkout", "main"]);
        git(&rival, &["config", "user.name", "Test"]);
        git(&rival, &["config", "user.email", "t@t"]);
        std::fs::write(rival.join("README.md"), "rival version\n").unwrap();
        git(&rival, &["add", "README.md"]);
        git(&rival, &["commit", "-m", "rival edit"]);
        git(&rival, &["push", "origin", "main"]);

        // Create a conflicting local commit on main (same file, different content)
        std::fs::write(local.join("README.md"), "local version\n").unwrap();
        git(&local, &["add", "README.md"]);
        git(&local, &["commit", "-m", "local edit"]);

        // Set last_fetch to a recent value so we can verify it gets invalidated
        *mgr.last_fetch.lock().unwrap() = Some(Instant::now());

        // resolve_and_push_main should fail because pull --rebase hits a conflict
        let result = mgr.resolve_and_push_main();
        assert!(result.is_err(), "should fail due to rebase conflict");

        // REBASE_HEAD should NOT exist — rebase must have been aborted
        let rebase_head = local.join(".git/REBASE_HEAD");
        assert!(
            !rebase_head.exists(),
            "REBASE_HEAD should not exist after rebase abort, repo is in dirty state"
        );

        // last_fetch should have been invalidated
        let last = mgr.last_fetch.lock().unwrap();
        assert!(
            last.is_none(),
            "last_fetch should be invalidated (None) after rebase failure"
        );
    }

    // ── Test 6: auto_push=false skips push ──────────────────────────

    #[test]
    fn merge_to_main_skips_push_when_auto_push_disabled() {
        let (origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);
        let task_id = "abcd-1234-ef56-7890-abcd-ef12";

        create_task_branch(&local, task_id);

        // Disable auto_push
        mgr.set_auto_push(false);

        // Record origin main HEAD before merge
        let origin_head_before = git(&origin, &["rev-parse", "main"]).trim().to_string();

        // Merge should succeed (no push error because push is skipped)
        mgr.merge_to_main(task_id).unwrap();

        // The task file should be on local main
        let log = git(&local, &["log", "--oneline", "main"]);
        assert!(
            log.contains("task work"),
            "merge commit should be on local main"
        );

        // Origin should NOT have the merge (push was skipped)
        let origin_head_after = git(&origin, &["rev-parse", "main"]).trim().to_string();
        assert_eq!(
            origin_head_before, origin_head_after,
            "origin main should not have changed when auto_push is disabled"
        );

        // Branch should still be deleted locally
        let branch = TaskId::new(task_id).branch_name();
        let branch_exists = Command::new("git")
            .arg("-C")
            .arg(&local)
            .args(["rev-parse", "--verify", &branch])
            .output()
            .unwrap()
            .status
            .success();
        assert!(!branch_exists, "branch should be deleted after merge");
    }

    // ── Test 7: Merge conflict aborts cleanly ───────────────────────

    #[test]
    fn merge_to_main_conflict_aborts() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);
        let task_id = "cccc-dddd-eeee-ffff-0000-1111";
        let tid = TaskId::new(task_id);
        let branch = tid.branch_name();

        // Create conflicting changes: modify same file on main and branch
        std::fs::write(local.join("README.md"), "main version\n").unwrap();
        git(&local, &["add", "README.md"]);
        git(
            &local,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "main edit",
            ],
        );
        git(&local, &["push", "origin", "main"]);

        // Create branch from before the main edit
        git(&local, &["checkout", "-b", &branch, "HEAD~1"]);
        std::fs::write(local.join("README.md"), "branch version\n").unwrap();
        git(&local, &["add", "README.md"]);
        git(
            &local,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "branch edit",
            ],
        );
        git(&local, &["checkout", "main"]);

        // merge_to_main should fail with a merge conflict error
        let result = mgr.merge_to_main(task_id);
        assert!(result.is_err(), "merge with conflict should return error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("merge conflict"),
            "error should mention merge conflict, got: {err_msg}"
        );
    }

    // ── Test 8: cleanup_merged_branches deletes merged local branches ──

    #[test]
    fn cleanup_merged_branches_deletes_merged_local() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);

        // Create two task branches and merge them manually (simulating merge_to_main
        // where branch deletion failed)
        let task_a = "aaaa-0001-0001-0001-0001-0001";
        let task_b = "bbbb-0002-0002-0002-0002-0002";
        create_task_branch(&local, task_a);
        create_task_branch(&local, task_b);

        let branch_a = TaskId::new(task_a).branch_name();
        let branch_b = TaskId::new(task_b).branch_name();

        // Merge both into main but do NOT delete the branches (simulating leftover)
        git(&local, &["merge", "--no-edit", &branch_a]);
        git(&local, &["merge", "--no-edit", &branch_b]);

        // Both branches should still exist
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &branch_a])
                .output()
                .unwrap()
                .status
                .success(),
            "branch_a should exist before cleanup"
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &branch_b])
                .output()
                .unwrap()
                .status
                .success(),
            "branch_b should exist before cleanup"
        );

        // Run cleanup
        mgr.cleanup_merged_branches();

        // Both branches should be gone
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &branch_a])
                .output()
                .unwrap()
                .status
                .success(),
            "branch_a should be deleted after cleanup"
        );
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &branch_b])
                .output()
                .unwrap()
                .status
                .success(),
            "branch_b should be deleted after cleanup"
        );
    }

    // ── Test 9: cleanup_merged_branches preserves unmerged branches ──

    #[test]
    fn cleanup_merged_branches_preserves_unmerged() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);

        // Create a merged branch and an unmerged branch
        let merged_task = "cccc-0003-0003-0003-0003-0003";
        let unmerged_task = "dddd-0004-0004-0004-0004-0004";
        create_task_branch(&local, merged_task);
        create_task_branch(&local, unmerged_task);

        let merged_branch = TaskId::new(merged_task).branch_name();
        let unmerged_branch = TaskId::new(unmerged_task).branch_name();

        // Only merge the first branch
        git(&local, &["merge", "--no-edit", &merged_branch]);

        // Run cleanup
        mgr.cleanup_merged_branches();

        // Merged branch should be deleted
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &merged_branch])
                .output()
                .unwrap()
                .status
                .success(),
            "merged branch should be deleted"
        );

        // Unmerged branch should still exist
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &unmerged_branch])
                .output()
                .unwrap()
                .status
                .success(),
            "unmerged branch should be preserved"
        );
    }

    // ── Test 10: cleanup_merged_branches deletes merged remote branches ──

    #[test]
    fn cleanup_merged_branches_deletes_merged_remote() {
        let (origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);
        mgr.set_auto_push(true);

        // Create a task branch, push it to origin, then merge locally
        let task_id = "eeee-0005-0005-0005-0005-0005";
        create_task_branch(&local, task_id);
        let branch = TaskId::new(task_id).branch_name();

        // Push branch to origin
        git(&local, &["push", "origin", &branch]);

        // Merge into main locally
        git(&local, &["merge", "--no-edit", &branch]);

        // Verify remote branch exists
        git(&local, &["fetch", "--prune"]);
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &format!("origin/{branch}")])
                .output()
                .unwrap()
                .status
                .success(),
            "remote branch should exist before cleanup"
        );

        // Run cleanup
        mgr.cleanup_merged_branches();

        // Local branch should be gone
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &branch])
                .output()
                .unwrap()
                .status
                .success(),
            "local branch should be deleted after cleanup"
        );

        // Remote branch should be gone too
        git(&local, &["fetch", "--prune"]);
        assert!(
            !Command::new("git")
                .arg("-C")
                .arg(&local)
                .args(["rev-parse", "--verify", &format!("origin/{branch}")])
                .output()
                .unwrap()
                .status
                .success(),
            "remote branch should be deleted after cleanup"
        );

        // Verify in origin directly
        let origin_branches = git(&origin, &["branch", "-a"]);
        assert!(
            !origin_branches.contains("agent/task-"),
            "origin should have no agent/task- branches, got: {origin_branches}"
        );
    }

    // ── Test 11: cleanup_merged_branches no-op when git disabled ──

    #[test]
    fn cleanup_merged_branches_noop_when_disabled() {
        let tmp = tempdir().unwrap();
        let mgr = WorktreeManager::disabled(tmp.path());
        // Should not panic
        mgr.cleanup_merged_branches();
    }

    // ── Test 12: merge creates target branch when it doesn't exist ──

    #[test]
    fn merge_to_branch_creates_missing_target() {
        let (_origin, local, _tmp) = setup_repos();
        // Manager targeting "dev" — but local repo only has "main"
        let mgr = WorktreeManager::with_branch(&local, "dev");

        let task_id = "aaaa-bbbb-cccc-dddd-eeee-ffff";
        create_task_branch(&local, task_id);

        // "dev" branch doesn't exist locally
        let dev_exists = Command::new("git")
            .arg("-C")
            .arg(&local)
            .args(["rev-parse", "--verify", "dev"])
            .output()
            .unwrap()
            .status
            .success();
        assert!(!dev_exists, "dev branch should NOT exist before merge");

        // Merge to dev should succeed — it creates the branch from HEAD
        mgr.merge_to_branch(task_id, "dev").unwrap();

        // dev branch should now exist
        let dev_exists_after = Command::new("git")
            .arg("-C")
            .arg(&local)
            .args(["rev-parse", "--verify", "dev"])
            .output()
            .unwrap()
            .status
            .success();
        assert!(dev_exists_after, "dev branch should exist after merge");

        // Task work should be on dev
        let log = git(&local, &["log", "--oneline", "dev"]);
        assert!(log.contains("task work"), "task commit should be on dev");
    }

    // ── Test 13: revert_task reverts a merge commit ──

    #[test]
    fn revert_task_reverts_merge_commit() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);

        let task_id = "ffff-0006-0006-0006-0006-0006";
        create_task_branch(&local, task_id);
        let tid = TaskId::new(task_id);

        // Create a second commit on main to force non-FF merge
        std::fs::write(local.join("other.txt"), "other work\n").unwrap();
        git(&local, &["add", "other.txt"]);
        git(
            &local,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "other work on main",
            ],
        );

        // Merge the task branch (non-FF, creates merge commit)
        let branch = tid.branch_name();
        git(&local, &["merge", "--no-edit", &branch]);

        // Verify the task file exists
        let task_file = format!("{}.txt", tid.short());
        assert!(
            local.join(&task_file).exists(),
            "task file should exist after merge"
        );

        // Revert the task
        let result = mgr.revert_task(task_id);
        assert!(result.is_ok(), "revert_task should succeed: {:?}", result);
        let msg = result.unwrap();
        assert!(
            msg.contains("Reverted merge commit"),
            "should report merge commit revert, got: {msg}"
        );

        // Verify the task file content is removed by the revert
        // (git revert creates a new commit undoing changes, file may still
        // exist but content should be removed or file deleted depending on merge)
        let log = git(&local, &["log", "--oneline", "main"]);
        assert!(
            log.contains("Revert"),
            "revert commit should be in history: {log}"
        );
    }

    // ── Test 14: revert_task reverts fast-forward commits ──

    #[test]
    fn revert_task_reverts_ff_commits() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);

        let task_id = "aaaa-0007-0007-0007-0007-0007";
        create_task_branch(&local, task_id);
        let tid = TaskId::new(task_id);

        // FF merge (main hasn't diverged)
        let branch = tid.branch_name();
        git(&local, &["merge", "--no-edit", &branch]);

        // Verify the task file exists
        let task_file = format!("{}.txt", tid.short());
        assert!(
            local.join(&task_file).exists(),
            "task file should exist after merge"
        );

        // Revert the task (should use Strategy 2: individual commits)
        let result = mgr.revert_task(task_id);
        assert!(result.is_ok(), "revert_task should succeed: {:?}", result);
        let msg = result.unwrap();
        assert!(
            msg.contains("Reverted 1 commit"),
            "should report commit revert, got: {msg}"
        );

        // Verify the revert commit exists
        let log = git(&local, &["log", "--oneline", "main"]);
        assert!(
            log.contains("Revert"),
            "revert commit should be in history: {log}"
        );
    }

    // ── Test 15: revert_task with auto_push pushes to origin ──

    #[test]
    fn revert_task_pushes_when_auto_push_enabled() {
        let (origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);
        mgr.set_auto_push(true);

        let task_id = "bbbb-0008-0008-0008-0008-0008";
        create_task_branch(&local, task_id);
        let tid = TaskId::new(task_id);

        // Create divergence on main to force merge commit
        std::fs::write(local.join("diverge.txt"), "diverge\n").unwrap();
        git(&local, &["add", "diverge.txt"]);
        git(
            &local,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "diverge on main",
            ],
        );
        // Push main first so we have an up-to-date remote
        git(&local, &["push", "origin", "main"]);

        // Merge the task branch
        let branch = tid.branch_name();
        git(&local, &["merge", "--no-edit", &branch]);
        git(&local, &["push", "origin", "main"]);

        // Now revert — should push the revert commit to origin
        let result = mgr.revert_task(task_id);
        assert!(result.is_ok(), "revert_task should succeed: {:?}", result);

        // Check that origin has the revert commit
        let origin_log = git(&origin, &["log", "--oneline", "main"]);
        assert!(
            origin_log.contains("Revert"),
            "origin should have revert commit: {origin_log}"
        );
    }

    // ── Test 16: revert_task fails for unmerged task ──

    #[test]
    fn revert_task_fails_for_unmerged_task() {
        let (_origin, local, _tmp) = setup_repos();
        let mgr = make_manager(&local);

        // Use a task ID that was never merged
        let result = mgr.revert_task("cccc-9999-9999-9999-9999-9999");
        assert!(result.is_err(), "revert_task should fail for unmerged task");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("No commits found"),
            "error should mention no commits, got: {err}"
        );
    }
}
