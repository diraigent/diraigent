use crate::git::WorktreeManager;
use crate::project::api::ProjectsApi;
use base64::Engine as _;
use serde::Serialize;
use std::process::Command;
use tracing::warn;

const MAX_BLOB_SIZE: usize = 1024 * 1024; // 1 MB

// ── Response type ──

#[derive(Debug, Serialize)]
pub struct GitResponse {
    pub success: bool,
    pub error: Option<String>,
    pub data: serde_json::Value,
}

// ── Pure git request handler (no transport dependency) ──

#[allow(clippy::too_many_arguments)]
pub fn handle_git_request(
    wm: &WorktreeManager,
    query_type: &str,
    prefix: Option<&str>,
    task_id: Option<&str>,
    branch: Option<&str>,
    remote: Option<&str>,
    path: Option<&str>,
    git_ref: Option<&str>,
) -> GitResponse {
    let (qt, result) = match query_type {
        "list_branches" => {
            let prefix = prefix.unwrap_or("agent/");
            (
                "list_branches",
                wm.list_branches(prefix)
                    .map(|r| serde_json::to_value(r).unwrap_or_default()),
            )
        }
        "task_branch_status" => {
            let task_id = match task_id {
                Some(id) => id,
                None => {
                    return GitResponse {
                        success: false,
                        error: Some("missing task_id".into()),
                        data: serde_json::Value::Null,
                    };
                }
            };
            (
                "task_branch_status",
                wm.task_branch_status(task_id)
                    .map(|r| serde_json::to_value(r).unwrap_or_default()),
            )
        }
        "main_status" => (
            "main_status",
            wm.main_push_status()
                .map(|r| serde_json::to_value(r).unwrap_or_default()),
        ),
        "push_main" => (
            "push_main",
            wm.push_main()
                .map(|msg| serde_json::json!({ "message": msg })),
        ),
        "resolve_and_push_main" => (
            "resolve_and_push_main",
            wm.resolve_and_push_main()
                .map(|msg| serde_json::json!({ "message": msg })),
        ),
        "push" => {
            let branch = match branch {
                Some(b) => b,
                None => {
                    return GitResponse {
                        success: false,
                        error: Some("missing branch".into()),
                        data: serde_json::Value::Null,
                    };
                }
            };
            let remote = remote.unwrap_or("origin");
            (
                "push",
                wm.push_branch(branch, remote)
                    .map(|msg| serde_json::json!({ "message": msg })),
            )
        }
        "revert_task" => {
            let task_id = match task_id {
                Some(id) => id,
                None => {
                    return GitResponse {
                        success: false,
                        error: Some("missing task_id".into()),
                        data: serde_json::Value::Null,
                    };
                }
            };
            (
                "revert_task",
                wm.revert_task(task_id)
                    .map(|msg| serde_json::json!({ "message": msg })),
            )
        }
        "resolve_task_branch" => {
            let task_id = match task_id {
                Some(id) => id,
                None => {
                    return GitResponse {
                        success: false,
                        error: Some("missing task_id".into()),
                        data: serde_json::Value::Null,
                    };
                }
            };
            (
                "resolve_task_branch",
                wm.resolve_task_branch(task_id)
                    .map(|msg| serde_json::json!({ "message": msg })),
            )
        }
        "release" => (
            "release",
            wm.release(branch, path, remote)
                .map(|msg| serde_json::json!({ "message": msg })),
        ),
        "source_tree" => {
            let git_ref = git_ref.unwrap_or(wm.default_branch());
            let path = path.unwrap_or("");
            return handle_source_tree(wm, git_ref, path);
        }
        "source_blob" => {
            let git_ref = git_ref.unwrap_or(wm.default_branch());
            let path = path.unwrap_or("");
            return handle_source_blob(wm, git_ref, path);
        }
        other => {
            return GitResponse {
                success: false,
                error: Some(format!("unknown query_type: {other}")),
                data: serde_json::Value::Null,
            };
        }
    };

    match result {
        Ok(data) => GitResponse {
            success: true,
            error: None,
            data,
        },
        Err(e) => {
            warn!(query_type = qt, error = %e, "git operation failed");
            GitResponse {
                success: false,
                error: Some(format!("{e:#}")),
                data: serde_json::Value::Null,
            }
        }
    }
}

/// Wrapper around `handle_git_request` that emits events to the API
/// after mutating git operations (push, revert, release).
///
/// Must be called from within a `spawn_blocking` context that has a
/// tokio runtime handle available (via `Handle::current()`).
#[allow(clippy::too_many_arguments)]
pub fn handle_git_request_with_events(
    wm: &WorktreeManager,
    api: &ProjectsApi,
    project_id: &str,
    query_type: &str,
    prefix: Option<&str>,
    task_id: Option<&str>,
    branch: Option<&str>,
    remote: Option<&str>,
    path: Option<&str>,
    git_ref: Option<&str>,
) -> GitResponse {
    let response = handle_git_request(
        wm, query_type, prefix, task_id, branch, remote, path, git_ref,
    );

    // Only emit events for mutating operations
    let event = build_git_event(api, query_type, &response, wm, task_id, branch);
    if let Some(event) = event
        && let Ok(rt) = tokio::runtime::Handle::try_current()
        && let Err(e) = rt.block_on(api.post_event(project_id, &event))
    {
        warn!(query_type, "failed to emit git event: {e}");
    }

    response
}

/// Build an event JSON value for mutating git operations, if applicable.
fn build_git_event(
    api: &ProjectsApi,
    query_type: &str,
    response: &GitResponse,
    wm: &WorktreeManager,
    task_id: Option<&str>,
    branch: Option<&str>,
) -> Option<serde_json::Value> {
    match query_type {
        "revert_task" if response.success => {
            let tid = task_id.unwrap_or("unknown");
            let revert_msg = response
                .data
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            Some(serde_json::json!({
                "kind": "custom",
                "source": "orchestra",
                "title": format!("Task reverted: {tid}"),
                "severity": "info",
                "related_task_id": tid,
                "agent_id": api.agent_id(),
                "metadata": {
                    "task_id": tid,
                    "revert_message": revert_msg,
                }
            }))
        }
        "push_main" | "resolve_and_push_main" => {
            let branch_name = wm.default_branch();
            if response.success {
                Some(serde_json::json!({
                    "kind": "custom",
                    "source": "orchestra",
                    "title": format!("Pushed {branch_name} to origin"),
                    "severity": "info",
                    "agent_id": api.agent_id(),
                    "metadata": {
                        "branch": branch_name,
                        "success": true,
                    }
                }))
            } else {
                Some(serde_json::json!({
                    "kind": "custom",
                    "source": "orchestra",
                    "title": format!("Push failed: {branch_name}"),
                    "severity": "warning",
                    "agent_id": api.agent_id(),
                    "metadata": {
                        "branch": branch_name,
                        "success": false,
                        "error": response.error.as_deref().unwrap_or("unknown error"),
                    }
                }))
            }
        }
        "push" => {
            let branch_name = branch.unwrap_or("unknown");
            if response.success {
                Some(serde_json::json!({
                    "kind": "custom",
                    "source": "orchestra",
                    "title": format!("Pushed {branch_name}"),
                    "severity": "info",
                    "agent_id": api.agent_id(),
                    "metadata": {
                        "branch": branch_name,
                        "success": true,
                    }
                }))
            } else {
                Some(serde_json::json!({
                    "kind": "custom",
                    "source": "orchestra",
                    "title": format!("Push failed: {branch_name}"),
                    "severity": "warning",
                    "agent_id": api.agent_id(),
                    "metadata": {
                        "branch": branch_name,
                        "success": false,
                        "error": response.error.as_deref().unwrap_or("unknown error"),
                    }
                }))
            }
        }
        "release" if response.success => {
            let release_msg = response
                .data
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            // Extract tag from the release message ("Released vYYYYMMDD-NN (N commits from ...)")
            let tag = release_msg.split_whitespace().nth(1).unwrap_or("unknown");
            Some(serde_json::json!({
                "kind": "release",
                "source": "orchestra",
                "title": format!("Release: {tag}"),
                "severity": "info",
                "agent_id": api.agent_id(),
                "metadata": {
                    "tag": tag,
                    "message": release_msg,
                }
            }))
        }
        _ => None,
    }
}

// ── Source browsing handlers ──

fn git_root_or_err(wm: &WorktreeManager) -> Result<&std::path::Path, GitResponse> {
    wm.git_root().ok_or_else(|| GitResponse {
        success: false,
        error: Some("git operations disabled for this project".into()),
        data: serde_json::Value::Null,
    })
}

fn handle_source_tree(wm: &WorktreeManager, git_ref: &str, path: &str) -> GitResponse {
    let repo_root = match git_root_or_err(wm) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Verify the directory actually exists and is a git repo
    if !repo_root.exists() {
        warn!(
            repo_root = %repo_root.display(),
            "source_tree: repo_root does not exist"
        );
        return GitResponse {
            success: false,
            error: Some(format!(
                "Repository path does not exist: {}",
                repo_root.display()
            )),
            data: serde_json::json!({ "entries": [] }),
        };
    }

    let mut args = vec!["ls-tree", "-z", git_ref];
    let pathspec;
    if !path.is_empty() {
        pathspec = format!("{path}/");
        args.push(&pathspec);
    }

    let output = match Command::new("git")
        .args(&args)
        .current_dir(repo_root)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return GitResponse {
                success: false,
                error: Some(format!("failed to run git ls-tree: {e}")),
                data: serde_json::Value::Null,
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Not a valid object name") || stderr.contains("fatal: bad revision") {
            return GitResponse {
                success: true,
                error: None,
                data: serde_json::json!({
                    "not_found": true,
                    "error": format!("Ref '{git_ref}' not found"),
                }),
            };
        }
        warn!(
            repo_root = %repo_root.display(),
            git_ref = %git_ref,
            path = %path,
            stderr = %stderr.trim(),
            "source_tree: git ls-tree failed"
        );
        // Return empty list but include error detail for debugging.
        return GitResponse {
            success: true,
            error: Some(format!("git ls-tree failed: {}", stderr.trim())),
            data: serde_json::json!({ "entries": [] }),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Each NUL-separated entry: "<mode> <type> <hash>\t<path>"
    // When a pathspec is given (e.g. `git ls-tree HEAD apps/`), the output
    // paths already include the parent prefix (e.g. `apps/api`).  We use the
    // full path directly and extract only the final component as the display
    // name so the frontend tree can match parent→child correctly.
    let entries: Vec<serde_json::Value> = stdout
        .split('\0')
        .filter(|s| !s.is_empty())
        .filter_map(|line| {
            let tab = line.find('\t')?;
            let meta = &line[..tab];
            let full_path = &line[tab + 1..];

            let kind = if meta.contains(" tree ") {
                "dir"
            } else {
                "file"
            };

            // full_path is already the complete path from the repo root
            // (e.g. "apps/api" when listing apps/), so use it directly.
            let display_name = match full_path.rfind('/') {
                Some(pos) => &full_path[pos + 1..],
                None => full_path,
            };

            Some(serde_json::json!({
                "name": display_name,
                "path": full_path,
                "kind": kind,
            }))
        })
        .collect();

    GitResponse {
        success: true,
        error: None,
        data: serde_json::json!({ "entries": entries }),
    }
}

fn handle_source_blob(wm: &WorktreeManager, git_ref: &str, path: &str) -> GitResponse {
    let repo_root = match git_root_or_err(wm) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let refpath = if path.is_empty() {
        git_ref.to_string()
    } else {
        format!("{git_ref}:{path}")
    };

    let output = match Command::new("git")
        .args(["show", &refpath])
        .current_dir(repo_root)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return GitResponse {
                success: false,
                error: Some(format!("failed to run git show: {e}")),
                data: serde_json::Value::Null,
            };
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not a valid object name")
            || stderr.contains("does not exist")
            || stderr.contains("bad revision")
            || stderr.contains("Not a valid object name")
        {
            return GitResponse {
                success: true,
                error: None,
                data: serde_json::json!({
                    "not_found": true,
                    "error": format!("'{path}' not found at ref '{git_ref}'"),
                }),
            };
        }
        return GitResponse {
            success: false,
            error: Some(format!("git show failed: {stderr}")),
            data: serde_json::Value::Null,
        };
    }

    let bytes = &output.stdout;
    let size = bytes.len();

    if size > MAX_BLOB_SIZE {
        return GitResponse {
            success: true,
            error: Some(format!("File exceeds 1 MB limit ({size} bytes)")),
            data: serde_json::json!({
                "too_large": true,
                "error": format!("File exceeds 1 MB limit ({size} bytes)"),
            }),
        };
    }

    let (content, encoding) = match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_string(), "utf8"),
        Err(_) => (
            base64::engine::general_purpose::STANDARD.encode(bytes),
            "base64",
        ),
    };

    GitResponse {
        success: true,
        error: None,
        data: serde_json::json!({
            "content": content,
            "encoding": encoding,
            "size": size,
        }),
    }
}
