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
    /// Branch from a target (defaults to `default_branch`), merge back on completion.
    /// Push controlled by project's `auto_push` setting.
    /// When `target_branch` is None, merges to default_branch.
    Merge { target_branch: Option<String> },

    /// Branch from `default_branch`, push task branch only (no merge).
    /// For PR-based workflows where a human reviews before merging.
    BranchOnly,

    /// Goal-based feature branches. Tasks branch from and merge into a goal branch
    /// (e.g. `goal/<slug>`). The goal branch itself is merged to default when the
    /// goal is completed.
    FeatureBranch { goal_branch: String },

    /// No git operations. Plain directory, no branching/merging/pushing.
    NoGit,
}

impl GitStrategy {
    /// Strategy ID as stored in playbook metadata.
    pub fn id(&self) -> &'static str {
        match self {
            GitStrategy::Merge { .. } => "merge",
            GitStrategy::BranchOnly => "branch_only",
            GitStrategy::FeatureBranch { .. } => "feature_branch",
            GitStrategy::NoGit => "no_git",
        }
    }

    /// Resolve strategy from playbook metadata JSON.
    ///
    /// Falls back to `Merge` (to default) for git-enabled projects,
    /// `NoGit` when `project_git_mode` is `"none"`.
    ///
    /// `goal_branch` is provided externally (from task→goal lookup) and
    /// only used when the strategy is `feature_branch`.
    pub fn from_playbook_metadata(
        metadata: &Value,
        project_git_mode: &str,
        goal_branch: Option<String>,
    ) -> Self {
        if project_git_mode == "none" {
            return GitStrategy::NoGit;
        }
        match metadata.get("git_strategy").and_then(|v| v.as_str()) {
            // Accept both old and new names for backwards compat
            Some("merge") | Some("merge_to_default") | None => {
                let target = metadata
                    .get("git_target_branch")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                GitStrategy::Merge {
                    target_branch: target,
                }
            }
            Some("branch_only") => GitStrategy::BranchOnly,
            // Accept old name too
            Some("branch_to_target") => {
                let target = metadata
                    .get("git_target_branch")
                    .and_then(|v| v.as_str())
                    .unwrap_or("develop")
                    .to_string();
                GitStrategy::Merge {
                    target_branch: Some(target),
                }
            }
            Some("feature_branch") => {
                if let Some(branch) = goal_branch {
                    GitStrategy::FeatureBranch {
                        goal_branch: branch,
                    }
                } else {
                    // No goal linked — fall back to merge-to-default
                    tracing::warn!(
                        "feature_branch strategy but no goal linked to task — falling back to merge"
                    );
                    GitStrategy::Merge {
                        target_branch: None,
                    }
                }
            }
            Some("no_git") => GitStrategy::NoGit,
            Some(_unknown) => GitStrategy::Merge {
                target_branch: None,
            },
        }
    }

    /// Which branch to base the worktree from.
    /// Returns `None` for `NoGit`.
    pub fn base_branch<'a>(&'a self, default_branch: &'a str) -> Option<&'a str> {
        match self {
            GitStrategy::Merge {
                target_branch: Some(t),
            } => Some(t.as_str()),
            GitStrategy::Merge {
                target_branch: None,
            }
            | GitStrategy::BranchOnly => Some(default_branch),
            GitStrategy::FeatureBranch { goal_branch } => Some(goal_branch.as_str()),
            GitStrategy::NoGit => None,
        }
    }

    /// Whether to merge the task branch into a target on completion.
    pub fn should_merge(&self) -> bool {
        matches!(
            self,
            GitStrategy::Merge { .. } | GitStrategy::FeatureBranch { .. }
        )
    }

    /// Whether to push the task branch (without merging) on completion.
    pub fn should_push_branch(&self) -> bool {
        matches!(self, GitStrategy::BranchOnly)
    }

    /// The branch to merge into (when `should_merge` is true).
    pub fn merge_target<'a>(&'a self, default_branch: &'a str) -> Option<&'a str> {
        match self {
            GitStrategy::Merge {
                target_branch: Some(t),
            } => Some(t.as_str()),
            GitStrategy::Merge {
                target_branch: None,
            } => Some(default_branch),
            GitStrategy::FeatureBranch { goal_branch } => Some(goal_branch.as_str()),
            _ => None,
        }
    }

    /// Returns the static catalog of available strategies (for API responses).
    pub fn catalog() -> Vec<Value> {
        vec![
            serde_json::json!({
                "id": "merge",
                "name": "Merge",
                "description": "Branch from target (default branch unless overridden), merge back when done. Standard autonomous workflow.",
                "fields": { "git_target_branch": "string (optional, defaults to project default branch)" },
            }),
            serde_json::json!({
                "id": "branch_only",
                "name": "Branch Only (No Merge)",
                "description": "Branch from default, push branch to origin. No automatic merge. For PR-based workflows.",
            }),
            serde_json::json!({
                "id": "feature_branch",
                "name": "Feature Branch (Goal-based)",
                "description": "Tasks branch from and merge into a goal branch. The goal branch merges to default when the goal is completed.",
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
/// Returns `Merge { target_branch: None }` as fallback when the task has no
/// playbook or the playbook fetch fails.
pub async fn resolve_strategy(
    api: &crate::api::ProjectsApi,
    task_data: Option<&Value>,
    project_git_mode: &str,
) -> GitStrategy {
    if project_git_mode == "none" {
        return GitStrategy::NoGit;
    }

    let default_merge = GitStrategy::Merge {
        target_branch: None,
    };

    let Some(task) = task_data else {
        return default_merge;
    };

    let playbook_id = task["playbook_id"].as_str().unwrap_or("");
    if playbook_id.is_empty() {
        return default_merge;
    }

    match api.get_playbook(playbook_id).await {
        Ok(playbook) => {
            let metadata = &playbook["metadata"];

            // Only resolve goal branch if strategy is feature_branch
            let goal_branch = if metadata.get("git_strategy").and_then(|v| v.as_str())
                == Some("feature_branch")
            {
                resolve_goal_branch(api, task).await
            } else {
                None
            };

            GitStrategy::from_playbook_metadata(metadata, project_git_mode, goal_branch)
        }
        Err(e) => {
            tracing::warn!("failed to fetch playbook {playbook_id} for git strategy: {e}");
            default_merge
        }
    }
}

/// Derive the goal branch name for a task by looking up its linked goals.
///
/// Returns `Some("goal/<slug>")` if the task is linked to a goal,
/// `None` otherwise.
async fn resolve_goal_branch(api: &crate::api::ProjectsApi, task: &Value) -> Option<String> {
    let task_id = task["id"].as_str()?;
    let goal_ids = match api.get_task_goals(task_id).await {
        Ok(ids) if !ids.is_empty() => ids,
        Ok(_) => {
            tracing::warn!("feature_branch strategy but task {task_id} has no linked goals");
            return None;
        }
        Err(e) => {
            tracing::warn!("failed to fetch goal IDs for task {task_id}: {e}");
            return None;
        }
    };

    // Fetch the first goal's details to get its title
    match api.get_goal(&goal_ids[0]).await {
        Ok(goal) => {
            let title = goal["title"].as_str().unwrap_or("unnamed");
            Some(format!("goal/{}", slugify(title)))
        }
        Err(e) => {
            tracing::warn!("failed to fetch goal {} for branch name: {e}", goal_ids[0]);
            None
        }
    }
}

/// Simple slugification: lowercase, replace non-alphanumeric with dashes,
/// collapse multiple dashes, trim dashes, truncate to 50 chars.
fn slugify(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.len() > 50 {
        slug[..50].trim_end_matches('-').to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_when_no_metadata() {
        let meta = serde_json::json!({});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: None
            }
        );
    }

    #[test]
    fn no_git_when_project_mode_none() {
        let meta = serde_json::json!({"git_strategy": "merge"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "none", None),
            GitStrategy::NoGit
        );
    }

    #[test]
    fn branch_only_from_metadata() {
        let meta = serde_json::json!({"git_strategy": "branch_only"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::BranchOnly
        );
    }

    #[test]
    fn merge_with_target_branch() {
        let meta = serde_json::json!({
            "git_strategy": "merge",
            "git_target_branch": "staging"
        });
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: Some("staging".to_string())
            }
        );
    }

    #[test]
    fn merge_without_target_branch() {
        let meta = serde_json::json!({"git_strategy": "merge"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: None
            }
        );
    }

    #[test]
    fn old_merge_to_default_compat() {
        let meta = serde_json::json!({"git_strategy": "merge_to_default"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: None
            }
        );
    }

    #[test]
    fn old_branch_to_target_compat() {
        let meta = serde_json::json!({
            "git_strategy": "branch_to_target",
            "git_target_branch": "staging"
        });
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: Some("staging".to_string())
            }
        );
    }

    #[test]
    fn branch_to_target_defaults_to_develop() {
        let meta = serde_json::json!({"git_strategy": "branch_to_target"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: Some("develop".to_string())
            }
        );
    }

    #[test]
    fn feature_branch_with_goal() {
        let meta = serde_json::json!({"git_strategy": "feature_branch"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(
                &meta,
                "standalone",
                Some("goal/user-auth".to_string())
            ),
            GitStrategy::FeatureBranch {
                goal_branch: "goal/user-auth".to_string()
            }
        );
    }

    #[test]
    fn feature_branch_without_goal_falls_back_to_merge() {
        let meta = serde_json::json!({"git_strategy": "feature_branch"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: None
            }
        );
    }

    #[test]
    fn unknown_strategy_falls_back_to_merge() {
        let meta = serde_json::json!({"git_strategy": "yolo_deploy"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(&meta, "standalone", None),
            GitStrategy::Merge {
                target_branch: None
            }
        );
    }

    #[test]
    fn base_branch_merge_default() {
        let s = GitStrategy::Merge {
            target_branch: None,
        };
        assert_eq!(s.base_branch("main"), Some("main"));
        assert_eq!(s.merge_target("main"), Some("main"));
        assert!(s.should_merge());
        assert!(!s.should_push_branch());
    }

    #[test]
    fn base_branch_merge_with_target() {
        let s = GitStrategy::Merge {
            target_branch: Some("develop".to_string()),
        };
        assert_eq!(s.base_branch("main"), Some("develop"));
        assert_eq!(s.merge_target("main"), Some("develop"));
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
    fn base_branch_feature_branch() {
        let s = GitStrategy::FeatureBranch {
            goal_branch: "goal/user-auth".to_string(),
        };
        assert_eq!(s.base_branch("main"), Some("goal/user-auth"));
        assert_eq!(s.merge_target("main"), Some("goal/user-auth"));
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

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("User Auth System"), "user-auth-system");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("Add OAuth 2.0 support!"), "add-oauth-2-0-support");
    }

    #[test]
    fn slugify_long_title() {
        let long = "a".repeat(60);
        assert!(slugify(&long).len() <= 50);
    }
}
