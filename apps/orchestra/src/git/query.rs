use crate::task_id::TaskId;
use anyhow::Result;
use std::process::{Command, Stdio};
use tracing::{debug, info};

use super::{
    BranchInfoResult, BranchListResult, ChangedFile, GIT_NET_TIMEOUT_SECS, MainPushStatus,
    TaskBranchResult, WorktreeManager, parse_track_short,
};

impl WorktreeManager {
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

    pub(super) fn get_ahead_behind(&self, local: &str, remote: &str) -> (i32, i32) {
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
}
