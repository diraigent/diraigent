pub mod handler;
mod merge;
pub mod provisioner;
mod query;
mod release;
pub mod strategy;
mod worktree;

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tracing::warn;

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
    pub(super) git_root: Option<PathBuf>,
    pub(super) worktree_dir: PathBuf,
    /// Last time `git fetch` was run — used to throttle network calls.
    /// Shared across all WorktreeManager instances for the same repo root
    /// via `FETCH_REGISTRY`, so per-request instantiation doesn't bypass
    /// the cooldown.
    pub(super) last_fetch: Arc<Mutex<Option<Instant>>>,
    /// When true, merge operations push to origin after merging and pull
    /// remote changes before merge. Default false (opt-in).
    pub(super) auto_push: AtomicBool,
    /// The project's default branch (e.g. "main", "develop"). All merge/push
    /// operations target this branch instead of hardcoding "main".
    pub(super) default_branch: String,
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

    /// Run `git fetch --prune --quiet` at most once per cooldown period.
    ///
    /// The mutex is released before running `git fetch` to avoid blocking
    /// concurrent git operations while a slow fetch is in progress.
    pub(super) fn fetch_if_stale(&self) {
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

    pub(super) fn git(&self, args: &[&str]) -> Result<()> {
        self.run_git(args, GIT_CMD_TIMEOUT_SECS)?;
        Ok(())
    }

    /// Run a git command with a timeout. Returns stdout on success.
    pub(super) fn run_git(&self, args: &[&str], timeout_secs: u64) -> Result<String> {
        let root = self
            .git_root
            .as_ref()
            .expect("run_git called with git disabled");
        run_git_in(root, args, timeout_secs)
    }

    pub(super) fn git_output(&self, args: &[&str]) -> Result<String> {
        self.run_git(args, GIT_CMD_TIMEOUT_SECS)
    }
}

// ── Free functions ──

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

// ── Result types for git responses ──

#[derive(Debug, serde::Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub change_type: String,
    pub diff: Option<String>,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_id::TaskId;
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

    // ── Test 4: Rebase-fail fallthrough — merge rolls back on push failure ─

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

        // Record pre-merge HEAD
        let pre_merge_head = git(&local, &["rev-parse", "main"]).trim().to_string();

        // merge_to_main with auto_push=true: pull --rebase will fail (no origin), but
        // merge should proceed. push_branch_with_retry will also fail, so the function
        // returns Err overall and the local merge is rolled back.
        let result = mgr.merge_to_main(task_id);
        assert!(
            result.is_err(),
            "should return error because push to origin fails"
        );

        // The local main branch should have been reset to its pre-merge state
        // (no stale merge commit left behind).
        let post_head = git(&local, &["rev-parse", "main"]).trim().to_string();
        assert_eq!(
            pre_merge_head, post_head,
            "local main should be reset to pre-merge HEAD after push failure"
        );
        let log = git(&local, &["log", "--oneline", "main"]);
        assert!(
            !log.contains("task work"),
            "merge commit should NOT be on local main after rollback, got: {log}"
        );

        // The task branch should be preserved (not deleted) so it can be retried
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
            branch_exists,
            "task branch should be preserved after push failure for retry"
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
