//! Shared project path resolution.
//!
//! Single source of truth for computing `git_root`, `working_dir`, and related
//! fields from a project API record.  Used by `main`, `spawner`, `chat`, and
//! `ws_client` — eliminates the three previous duplicate implementations.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::git::provisioner as git_provisioner;
use crate::project::api::ProjectsApi;

/// Resolved filesystem paths for a project.
pub struct ProjectPaths {
    pub git_mode: String,
    pub git_root: Option<PathBuf>,
    pub working_dir: PathBuf,
    pub auto_push: bool,
    pub default_branch: String,
    /// When true, upload task execution logs to the API after each worker completes.
    pub upload_logs: bool,
    /// When true, collect and store per-file diffs after each worker completes.
    /// Disabled by default to save storage and processing time.
    pub store_diffs: bool,
}

/// Fetch the project record and resolve git paths based on `git_mode`.
///
/// Resolution logic:
/// - standalone: git_root = PROJECTS_PATH/git_root, working_dir = git_root
/// - monorepo:   git_root = PROJECTS_PATH/git_root, working_dir = git_root/project_root
/// - none:       git_root = None, working_dir = projects_path (git ops disabled)
pub async fn resolve_project_paths(
    api: &ProjectsApi,
    project_id: &str,
    projects_path: &Path,
) -> Result<ProjectPaths> {
    let project = api
        .get_project(project_id)
        .await
        .context("fetch project record for path resolution")?;

    let git_mode = project["git_mode"]
        .as_str()
        .unwrap_or("standalone")
        .to_string();

    let auto_push = project["metadata"]["auto_push"].as_bool().unwrap_or(false);
    let upload_logs = project["metadata"]["upload_logs"]
        .as_bool()
        .unwrap_or(false);
    let store_diffs = project["metadata"]["store_diffs"]
        .as_bool()
        .unwrap_or(false);
    let default_branch = project["default_branch"]
        .as_str()
        .unwrap_or("main")
        .to_string();

    if git_mode == "none" {
        return Ok(ProjectPaths {
            git_mode: "none".to_string(),
            git_root: None,
            working_dir: projects_path.to_path_buf(),
            auto_push,
            default_branch,
            upload_logs,
            store_diffs,
        });
    }

    let git_root_rel = project["git_root"].as_str().unwrap_or("");
    let project_root_rel = project["project_root"].as_str().unwrap_or("");
    let repo_url = project["repo_url"].as_str().unwrap_or("");
    let slug = project["slug"].as_str().unwrap_or("");

    // Must match the provisioner's target path logic:
    // 1. Explicit git_root  2. Derived from repo_url path  3. Slug fallback
    let git_root = if !git_root_rel.is_empty() {
        projects_path.join(git_root_rel)
    } else if let Some(repo_path) = git_provisioner::repo_path_from_url(repo_url) {
        projects_path.join(repo_path)
    } else if !slug.is_empty() {
        projects_path.join(slug)
    } else {
        projects_path.to_path_buf()
    };

    let working_dir = if git_mode == "monorepo" && !project_root_rel.is_empty() {
        git_root.join(project_root_rel)
    } else {
        git_root.clone()
    };

    Ok(ProjectPaths {
        git_mode,
        git_root: Some(git_root),
        working_dir,
        auto_push,
        default_branch,
        upload_logs,
        store_diffs,
    })
}

/// Create a [`WorktreeManager`] for a specific project by resolving its paths from the API.
///
/// Used by the scheduler to get a per-project WM at reap time, avoiding the
/// single-project assumption of a top-level WM.
pub async fn create_project_wm(
    api: &ProjectsApi,
    project_id: &str,
    projects_path: &Path,
) -> Result<crate::git::WorktreeManager> {
    let paths = resolve_project_paths(api, project_id, projects_path).await?;
    let wm = if paths.git_mode == "none" {
        crate::git::WorktreeManager::disabled(&paths.working_dir)
    } else if let Some(ref git_root) = paths.git_root {
        crate::git::WorktreeManager::with_branch(git_root, &paths.default_branch)
    } else {
        crate::git::WorktreeManager::disabled(&paths.working_dir)
    };
    wm.set_auto_push(paths.auto_push);
    Ok(wm)
}

/// Convenience wrapper that resolves only the working directory for a project.
///
/// Falls back to `projects_path` on error.
pub async fn resolve_working_dir(
    api: &ProjectsApi,
    project_id: &str,
    projects_path: &Path,
) -> Result<PathBuf> {
    let paths = resolve_project_paths(api, project_id, projects_path).await?;
    Ok(paths.working_dir)
}
