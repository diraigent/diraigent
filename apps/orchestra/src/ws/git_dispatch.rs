use crate::git::WorktreeManager;
use crate::project::api::ProjectsApi;
use crate::ws::WsSender;
use crate::ws::protocol::WsMessage;
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Parameters for a git request received over WebSocket.
pub struct GitRequestParams {
    pub sender: WsSender,
    pub request_id: String,
    pub project_id: Uuid,
    pub query_type: String,
    pub prefix: Option<String>,
    pub task_id: Option<String>,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub path: Option<String>,
    pub git_ref: Option<String>,
    pub api: ProjectsApi,
    pub projects_path: PathBuf,
}

/// Handle a git.request received over WebSocket by resolving project paths,
/// provisioning the repo if needed, and delegating to `git_handler`.
pub fn handle_git_request(params: GitRequestParams) {
    let GitRequestParams {
        sender,
        request_id,
        project_id,
        query_type,
        prefix,
        task_id,
        branch,
        remote,
        path,
        git_ref,
        api,
        projects_path: pp,
    } = params;

    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();

        // Resolve project paths (git_root + working_dir)
        let paths = rt.block_on(async {
            crate::project::paths::resolve_project_paths(&api, &project_id.to_string(), &pp).await
        });
        let (git_mode, git_root, working_dir, auto_push, default_branch) = match paths {
            Ok(p) => (
                p.git_mode,
                p.git_root,
                p.working_dir,
                p.auto_push,
                p.default_branch,
            ),
            Err(e) => {
                warn!(
                    project_id = %project_id,
                    error = %e,
                    "failed to resolve project paths, falling back to projects_path"
                );
                (
                    "standalone".to_string(),
                    Some(pp.clone()),
                    pp.clone(),
                    false,
                    "main".to_string(),
                )
            }
        };

        // For git_mode=none, skip all git operations
        let wm = if git_mode == "none" {
            WorktreeManager::disabled(&working_dir)
        } else {
            // Use git_root for provisioning and WorktreeManager;
            // working_dir may be a monorepo subdirectory.
            let root = git_root.as_deref().unwrap_or(&working_dir);

            // Auto-provision repo if it doesn't exist yet
            if !root.join(".git").exists() {
                info!(
                    project_id = %project_id,
                    git_root = %root.display(),
                    "git request: repo not found, provisioning..."
                );
                // Fetch project record for repo_url/slug
                if let Ok(project) = rt.block_on(api.get_project(&project_id.to_string())) {
                    let repo_url = project["repo_url"].as_str().unwrap_or("");
                    let slug = project["slug"].as_str().unwrap_or("");
                    crate::git::provisioner::provision_repo(root, repo_url, &default_branch, slug);
                }
            }

            let m = WorktreeManager::with_branch(root, &default_branch);
            m.set_auto_push(auto_push);
            m
        };
        let response = crate::git::handler::handle_git_request_with_events(
            &wm,
            &api,
            &project_id.to_string(),
            &query_type,
            prefix.as_deref(),
            task_id.as_deref(),
            branch.as_deref(),
            remote.as_deref(),
            path.as_deref(),
            git_ref.as_deref(),
        );

        let ws_response = WsMessage::GitResponse {
            request_id,
            success: response.success,
            error: response.error,
            data: response.data,
        };

        if let Err(e) = sender.send(ws_response) {
            error!("failed to send git response via WS: {e}");
        }
    });
}
