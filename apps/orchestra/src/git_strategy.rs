//! Immutable git strategies that control branch/merge/push behavior per-playbook.
//!
//! Strategies are fixed — users select one, they cannot define custom ones.
//! Resolution order: playbook metadata → project git_mode fallback.

use serde_json::Value;

/// Per-step git action: what git operation to perform after this step completes.
///
/// This allows mid-pipeline merges/pushes without dedicated "merge" steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitAction {
    /// No git action after this step.
    None,
    /// Merge the task branch into the strategy's target branch.
    Merge,
    /// Push the task branch to the remote.
    Push,
}

impl GitAction {
    /// Parse a `GitAction` from a playbook step JSON object.
    ///
    /// Reads the `"git_action"` field. Defaults to `None` if absent or unrecognised.
    pub fn from_step_json(step: &Value) -> Self {
        match step.get("git_action").and_then(|v| v.as_str()) {
            Some("merge") => GitAction::Merge,
            Some("push") => GitAction::Push,
            _ => GitAction::None,
        }
    }
}

/// Predefined git workflow strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStrategy {
    /// Branch from `default_branch`, merge back on completion.
    /// Push controlled by project's `auto_push` setting.
    MergeToDefault,

    /// Branch from `default_branch`, push task branch only (no merge).
    /// For PR-based workflows where a human reviews before merging.
    BranchOnly,

    /// Branch from a specific target branch (e.g. `develop`, `staging`),
    /// merge back to that same branch on completion.
    BranchToTarget { target_branch: String },

    /// No git operations. Plain directory, no branching/merging/pushing.
    NoGit,
}

impl GitStrategy {
    /// Strategy ID as stored in playbook metadata.
    pub fn id(&self) -> &'static str {
        match self {
            GitStrategy::MergeToDefault => "merge_to_default",
            GitStrategy::BranchOnly => "branch_only",
            GitStrategy::BranchToTarget { .. } => "branch_to_target",
            GitStrategy::NoGit => "no_git",
        }
    }

    /// Resolve strategy from playbook metadata JSON.
    ///
    /// Falls back to `MergeToDefault` for git-enabled projects,
    /// `NoGit` when `project_git_mode` is `"none"`.
    pub fn from_playbook_metadata(metadata: &Value, project_git_mode: &str) -> Self {
        if project_git_mode == "none" {
            return GitStrategy::NoGit;
        }
        match metadata.get("git_strategy").and_then(|v| v.as_str()) {
            Some("merge_to_default") | None => GitStrategy::MergeToDefault,
            Some("branch_only") => GitStrategy::BranchOnly,
            Some("branch_to_target") => {
                let target = metadata
                    .get("git_target_branch")
                    .and_then(|v| v.as_str())
                    .unwrap_or("develop")
                    .to_string();
                GitStrategy::BranchToTarget {
                    target_branch: target,
                }
            }
            Some("no_git") => GitStrategy::NoGit,
            Some(_unknown) => GitStrategy::MergeToDefault,
        }
    }

    /// Which branch to base the worktree from.
    /// Returns `None` for `NoGit`.
    pub fn base_branch<'a>(&'a self, default_branch: &'a str) -> Option<&'a str> {
        match self {
            GitStrategy::MergeToDefault | GitStrategy::BranchOnly => Some(default_branch),
            GitStrategy::BranchToTarget { target_branch } => Some(target_branch.as_str()),
            GitStrategy::NoGit => None,
        }
    }

    /// Whether to merge the task branch into a target on completion.
    pub fn should_merge(&self) -> bool {
        matches!(
            self,
            GitStrategy::MergeToDefault | GitStrategy::BranchToTarget { .. }
        )
    }

    /// Whether to push the task branch (without merging) on completion.
    pub fn should_push_branch(&self) -> bool {
        matches!(self, GitStrategy::BranchOnly)
    }

    /// The branch to merge into (when `should_merge` is true).
    pub fn merge_target<'a>(&'a self, default_branch: &'a str) -> Option<&'a str> {
        match self {
            GitStrategy::MergeToDefault => Some(default_branch),
            GitStrategy::BranchToTarget { target_branch } => Some(target_branch.as_str()),
            _ => None,
        }
    }

    /// Returns the static catalog of available strategies (for API responses).
    pub fn catalog() -> Vec<Value> {
        vec![
            serde_json::json!({
                "id": "merge_to_default",
                "name": "Merge to Default Branch",
                "description": "Branch from default, merge back when done. Standard autonomous workflow.",
            }),
            serde_json::json!({
                "id": "branch_only",
                "name": "Branch Only (No Merge)",
                "description": "Branch from default, push branch to origin. No automatic merge. For PR-based workflows.",
            }),
            serde_json::json!({
                "id": "branch_to_target",
                "name": "Merge to Target Branch",
                "description": "Branch from and merge to a specified target branch (e.g. develop, staging).",
                "fields": { "git_target_branch": "string (required)" },
            }),
            serde_json::json!({
                "id": "no_git",
                "name": "No Git",
                "description": "Plain directory, no git operations. For non-code tasks.",
            }),
        ]
    }
}

/// Resolve the git strategy for a task by fetching its playbook metadata.
///
/// Returns `MergeToDefault` as fallback when the task has no playbook or
/// the playbook fetch fails.
pub async fn resolve_strategy(
    api: &crate::api::ProjectsApi,
    task_data: Option<&Value>,
    project_git_mode: &str,
) -> GitStrategy {
    if project_git_mode == "none" {
        return GitStrategy::NoGit;
    }

    let Some(task) = task_data else {
        return GitStrategy::MergeToDefault;
    };

    let playbook_id = task["playbook_id"].as_str().unwrap_or("");
    if playbook_id.is_empty() {
        return GitStrategy::MergeToDefault;
    }

    match api.get_playbook(playbook_id).await {
        Ok(playbook) => {
            let metadata = &playbook["metadata"];
            GitStrategy::from_playbook_metadata(metadata, project_git_mode)
        }
        Err(e) => {
            tracing::warn!("failed to fetch playbook {playbook_id} for git strategy: {e}");
            GitStrategy::MergeToDefault
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_when_no_metadata() {
        let meta = serde_json::json!({});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone"),
            GitStrategy::MergeToDefault
        );
    }

    #[test]
    fn no_git_when_project_mode_none() {
        let meta = serde_json::json!({"git_strategy": "merge_to_default"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "none"),
            GitStrategy::NoGit
        );
    }

    #[test]
    fn branch_only_from_metadata() {
        let meta = serde_json::json!({"git_strategy": "branch_only"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone"),
            GitStrategy::BranchOnly
        );
    }

    #[test]
    fn branch_to_target_with_explicit_branch() {
        let meta = serde_json::json!({
            "git_strategy": "branch_to_target",
            "git_target_branch": "staging"
        });
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone"),
            GitStrategy::BranchToTarget {
                target_branch: "staging".to_string()
            }
        );
    }

    #[test]
    fn branch_to_target_defaults_to_develop() {
        let meta = serde_json::json!({"git_strategy": "branch_to_target"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone"),
            GitStrategy::BranchToTarget {
                target_branch: "develop".to_string()
            }
        );
    }

    #[test]
    fn unknown_strategy_falls_back_to_default() {
        let meta = serde_json::json!({"git_strategy": "yolo_deploy"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone"),
            GitStrategy::MergeToDefault
        );
    }

    #[test]
    fn base_branch_merge_to_default() {
        let s = GitStrategy::MergeToDefault;
        assert_eq!(s.base_branch("main"), Some("main"));
        assert_eq!(s.merge_target("main"), Some("main"));
        assert!(s.should_merge());
        assert!(!s.should_push_branch());
    }

    #[test]
    fn base_branch_branch_only() {
        let s = GitStrategy::BranchOnly;
        assert_eq!(s.base_branch("main"), Some("main"));
        assert_eq!(s.merge_target("main"), None);
        assert!(!s.should_merge());
        assert!(s.should_push_branch());
    }

    #[test]
    fn base_branch_to_target() {
        let s = GitStrategy::BranchToTarget {
            target_branch: "develop".to_string(),
        };
        assert_eq!(s.base_branch("main"), Some("develop"));
        assert_eq!(s.merge_target("main"), Some("develop"));
        assert!(s.should_merge());
        assert!(!s.should_push_branch());
    }

    #[test]
    fn no_git_has_no_branches() {
        let s = GitStrategy::NoGit;
        assert_eq!(s.base_branch("main"), None);
        assert_eq!(s.merge_target("main"), None);
        assert!(!s.should_merge());
        assert!(!s.should_push_branch());
    }

    #[test]
    fn catalog_has_4_entries() {
        assert_eq!(GitStrategy::catalog().len(), 4);
    }

    #[test]
    fn git_action_from_step_json_none_when_absent() {
        let step = serde_json::json!({"name": "implement"});
        assert_eq!(GitAction::from_step_json(&step), GitAction::None);
    }

    #[test]
    fn git_action_from_step_json_merge() {
        let step = serde_json::json!({"name": "implement", "git_action": "merge"});
        assert_eq!(GitAction::from_step_json(&step), GitAction::Merge);
    }

    #[test]
    fn git_action_from_step_json_push() {
        let step = serde_json::json!({"name": "implement", "git_action": "push"});
        assert_eq!(GitAction::from_step_json(&step), GitAction::Push);
    }

    #[test]
    fn git_action_from_step_json_unknown_defaults_to_none() {
        let step = serde_json::json!({"name": "implement", "git_action": "yolo"});
        assert_eq!(GitAction::from_step_json(&step), GitAction::None);
    }
}
