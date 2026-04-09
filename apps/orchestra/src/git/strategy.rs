//! Immutable git strategies that control branch/merge/push behavior per-playbook.
//!
//! Strategies are fixed — users select one, they cannot define custom ones.
//! Resolution order: playbook metadata → project git_mode fallback.

use serde_json::Value;

use crate::repo_playbooks;

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

    /// Work-based feature branches. Tasks branch from and merge into a work branch
    /// (e.g. `work/<slug>`). The work branch itself is merged to default when the
    /// work item is completed.
    FeatureBranch { work_branch: String },

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
    /// `work_branch` is provided externally (from task→work item lookup) and
    /// only used when the strategy is `feature_branch`.
    pub fn from_playbook_metadata(
        metadata: &Value,
        project_git_mode: &str,
        work_branch: Option<String>,
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
                if let Some(branch) = work_branch {
                    GitStrategy::FeatureBranch {
                        work_branch: branch,
                    }
                } else {
                    // No work item linked — fall back to merge-to-default
                    tracing::warn!(
                        "feature_branch strategy but no work item linked to task — falling back to merge"
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
            GitStrategy::FeatureBranch { work_branch } => Some(work_branch.as_str()),
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
            GitStrategy::FeatureBranch { work_branch } => Some(work_branch.as_str()),
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
                "name": "Feature Branch (Work-based)",
                "description": "Tasks branch from and merge into a work branch. The work branch merges to default when the work item is completed.",
            }),
            serde_json::json!({
                "id": "no_git",
                "name": "No Git",
                "description": "Plain directory, no git operations. For non-code tasks.",
            }),
        ]
    }
}

/// Resolve the git strategy for a task from repo/YAML playbook metadata.
///
/// Returns `Merge { target_branch: None }` as fallback when the task has no
/// playbook or the playbook fetch fails.
pub async fn resolve_strategy(
    api: &dyn crate::engine::task_source::TaskSource,
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

    let playbook_name = task["playbook_name"].as_str().unwrap_or("");
    if playbook_name.is_empty() {
        return default_merge;
    }

    let project_id = task["project_id"].as_str().unwrap_or("");
    let project = match api.get_project(project_id).await {
        Ok(project) => project,
        Err(e) => {
            tracing::warn!("failed to fetch project {project_id} for git strategy: {e}");
            return default_merge;
        }
    };
    let repo_root = project["git_resolved_path"]
        .as_str()
        .or_else(|| project["resolved_path"].as_str());
    let Some(repo_root) = repo_root else {
        return default_merge;
    };
    let Some(playbook) =
        repo_playbooks::find_playbook_by_name(std::path::Path::new(repo_root), playbook_name)
    else {
        tracing::warn!("failed to load playbook {playbook_name} for git strategy");
        return default_merge;
    };

    let metadata = &playbook.metadata;
    let work_branch = if metadata.get("git_strategy").and_then(|v| v.as_str())
        == Some("feature_branch")
    {
        resolve_work_branch(api, task).await
    } else {
        None
    };

    GitStrategy::from_playbook_metadata(metadata, project_git_mode, work_branch)
}

/// Derive the work branch name for a task by looking up its linked work items.
///
/// Returns `Some("work/<slug>")` if the task is linked to a work item,
/// `None` otherwise.
async fn resolve_work_branch(
    api: &dyn crate::engine::task_source::TaskSource,
    task: &Value,
) -> Option<String> {
    let task_id = task["id"].as_str()?;
    let work_ids = match api.get_task_work_items(task_id).await {
        Ok(ids) if !ids.is_empty() => ids,
        Ok(_) => {
            tracing::warn!("feature_branch strategy but task {task_id} has no linked work items");
            return None;
        }
        Err(e) => {
            tracing::warn!("failed to fetch work item IDs for task {task_id}: {e}");
            return None;
        }
    };

    // Fetch the first work item's details to get its title
    let work_id = work_ids[0].as_str().unwrap_or_else(|| {
        tracing::warn!("work item ID is not a string: {}", work_ids[0]);
        ""
    });
    if work_id.is_empty() {
        return None;
    }
    match api.get_work_item(work_id).await {
        Ok(work_item) => {
            let title = work_item["title"].as_str().unwrap_or("unnamed");
            Some(format!("work/{}", slugify(title)))
        }
        Err(e) => {
            tracing::warn!("failed to fetch work item {work_id} for branch name: {e}");
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
    fn feature_branch_with_work_item() {
        let meta = serde_json::json!({"git_strategy": "feature_branch"});
        assert_eq!(
            GitStrategy::from_playbook_metadata(
                &meta,
                "standalone",
                Some("work/user-auth".to_string())
            ),
            GitStrategy::FeatureBranch {
                work_branch: "work/user-auth".to_string()
            }
        );
    }

    #[test]
    fn feature_branch_without_work_item_falls_back_to_merge() {
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
            work_branch: "work/user-auth".to_string(),
        };
        assert_eq!(s.base_branch("main"), Some("work/user-auth"));
        assert_eq!(s.merge_target("main"), Some("work/user-auth"));
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
