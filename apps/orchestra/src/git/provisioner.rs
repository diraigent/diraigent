//! Git provisioner — clones or initializes git repos when projects are created/updated.
//!
//! Provisioning functions are called directly from the worker spawning logic
//! in main.rs.

use std::path::Path;
use std::process::Command;
use tracing::{error, info, warn};

pub fn provision_repo(target: &Path, repo_url: &str, default_branch: &str, slug: &str) {
    let has_git = target.join(".git").exists();

    if has_git {
        if !repo_url.is_empty() {
            // Already cloned — fetch latest
            info!("git provisioner [{slug}]: fetching {}", target.display());
            let output = Command::new("git")
                .args(["fetch", "--all"])
                .current_dir(target)
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    info!("git provisioner [{slug}]: fetch complete");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!("git provisioner [{slug}]: fetch failed: {stderr}");
                }
                Err(e) => {
                    error!("git provisioner [{slug}]: fetch error: {e}");
                }
            }
        }
        // Already exists, no repo_url → nothing to do
        return;
    }

    if !repo_url.is_empty() {
        // Target directory may exist but not be a git repo (e.g. leftover data dir).
        // Clone into a temporary sibling and rename to avoid "not an empty directory".
        if target.exists() {
            warn!(
                "git provisioner [{slug}]: {} exists but is not a git repo — cloning via temp dir",
                target.display()
            );
            let tmp_target = target.with_extension("git-clone-tmp");
            if tmp_target.exists() {
                std::fs::remove_dir_all(&tmp_target).ok();
            }
            let output = Command::new("git")
                .args([
                    "clone",
                    "--branch",
                    default_branch,
                    repo_url,
                    &tmp_target.to_string_lossy(),
                ])
                .output();
            // If clone with --branch fails, retry without it (branch may not
            // exist on the remote yet).
            let clone_ok = match &output {
                Ok(o) if o.status.success() => true,
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!(
                        "git provisioner [{slug}]: clone (tmp) with --branch {default_branch} failed: {stderr}"
                    );
                    std::fs::remove_dir_all(&tmp_target).ok();
                    false
                }
                Err(e) => {
                    error!("git provisioner [{slug}]: clone (tmp) error: {e}");
                    false
                }
            };
            let output = if clone_ok {
                output
            } else {
                info!("git provisioner [{slug}]: retrying clone (tmp) without --branch");
                if tmp_target.exists() {
                    std::fs::remove_dir_all(&tmp_target).ok();
                }
                Command::new("git")
                    .args(["clone", repo_url, &tmp_target.to_string_lossy()])
                    .output()
            };
            match output {
                Ok(o) if o.status.success() => {
                    // Move .git into the existing directory and checkout
                    let src_git = tmp_target.join(".git");
                    let dst_git = target.join(".git");
                    match std::fs::rename(&src_git, &dst_git) {
                        Ok(()) => {
                            std::fs::remove_dir_all(&tmp_target).ok();
                            // Reset working tree to match HEAD.
                            // Try checking out default_branch; if it doesn't
                            // exist (branch was missing on remote), create it.
                            let reset = Command::new("git")
                                .args(["checkout", "--force", default_branch])
                                .current_dir(target)
                                .output();
                            match reset {
                                Ok(r) if r.status.success() => {
                                    info!("git provisioner [{slug}]: clone (via merge) complete");
                                }
                                _ => {
                                    // Branch doesn't exist — create it from HEAD
                                    let create = Command::new("git")
                                        .args(["checkout", "-b", default_branch])
                                        .current_dir(target)
                                        .output();
                                    match create {
                                        Ok(c) if c.status.success() => {
                                            info!(
                                                "git provisioner [{slug}]: clone (via merge) complete, created branch {default_branch}"
                                            );
                                        }
                                        Ok(c) => {
                                            let stderr = String::from_utf8_lossy(&c.stderr);
                                            warn!(
                                                "git provisioner [{slug}]: checkout -b {default_branch} failed: {stderr}"
                                            );
                                        }
                                        Err(e) => {
                                            error!(
                                                "git provisioner [{slug}]: checkout -b {default_branch} error: {e}"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "git provisioner [{slug}]: failed to move .git into target: {e}"
                            );
                            std::fs::remove_dir_all(&tmp_target).ok();
                        }
                    }
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    error!("git provisioner [{slug}]: clone (tmp) failed: {stderr}");
                    std::fs::remove_dir_all(&tmp_target).ok();
                }
                Err(e) => {
                    error!("git provisioner [{slug}]: clone (tmp) error: {e}");
                }
            }
        } else {
            // Fresh clone — target doesn't exist yet
            info!(
                "git provisioner [{slug}]: cloning {repo_url} → {}",
                target.display()
            );
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let output = Command::new("git")
                .args([
                    "clone",
                    "--branch",
                    default_branch,
                    repo_url,
                    &target.to_string_lossy(),
                ])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    info!("git provisioner [{slug}]: clone complete");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!(
                        "git provisioner [{slug}]: clone with --branch {default_branch} failed: {stderr}"
                    );
                    // The branch may not exist on the remote yet (e.g. project
                    // default_branch was set to "dev" but only "main" exists).
                    // Fall back to a plain clone and create the branch locally.
                    info!("git provisioner [{slug}]: retrying clone without --branch");
                    let fallback = Command::new("git")
                        .args(["clone", repo_url, &target.to_string_lossy()])
                        .output();
                    match fallback {
                        Ok(fb) if fb.status.success() => {
                            info!("git provisioner [{slug}]: fallback clone complete");
                            // Create the desired default branch from HEAD
                            let branch_out = Command::new("git")
                                .args(["checkout", "-b", default_branch])
                                .current_dir(target)
                                .output();
                            match branch_out {
                                Ok(b) if b.status.success() => {
                                    info!(
                                        "git provisioner [{slug}]: created local branch {default_branch}"
                                    );
                                }
                                Ok(b) => {
                                    let bstderr = String::from_utf8_lossy(&b.stderr);
                                    warn!(
                                        "git provisioner [{slug}]: checkout -b {default_branch} failed: {bstderr}"
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "git provisioner [{slug}]: checkout -b {default_branch} error: {e}"
                                    );
                                }
                            }
                        }
                        Ok(fb) => {
                            let fb_stderr = String::from_utf8_lossy(&fb.stderr);
                            error!(
                                "git provisioner [{slug}]: fallback clone also failed: {fb_stderr}"
                            );
                        }
                        Err(e) => {
                            error!("git provisioner [{slug}]: fallback clone error: {e}");
                        }
                    }
                }
                Err(e) => {
                    error!("git provisioner [{slug}]: clone error: {e}");
                }
            }
        }
    } else {
        // Init
        info!(
            "git provisioner [{slug}]: initializing new repo at {}",
            target.display()
        );
        std::fs::create_dir_all(target).ok();
        let output = Command::new("git")
            .args(["init", "--initial-branch", default_branch])
            .current_dir(target)
            .output();
        match output {
            Ok(o) if o.status.success() => {
                info!("git provisioner [{slug}]: init complete");
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                error!("git provisioner [{slug}]: init failed: {stderr}");
            }
            Err(e) => {
                error!("git provisioner [{slug}]: init error: {e}");
            }
        }
    }
}

/// Extract host + path from a git repo URL, stripping the `.git` suffix.
/// Includes the domain to avoid collisions between different hosts.
///
/// Examples:
/// - `https://github.com/acme/widgets` → `"github.com/acme/widgets"`
/// - `https://github.com/acme/widgets.git` → `"github.com/acme/widgets"`
/// - `git@github.com:org/repo.git` → `"github.com/org/repo"`
pub fn repo_path_from_url(url: &str) -> Option<String> {
    let full = if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        // https://host/org/repo.git → keep host + path
        Some(rest)
    } else if let Some(colon) = url.find(':') {
        // git@host:org/repo.git → host + "/" + path
        let user_host = &url[..colon];
        let host = user_host.rsplit('@').next().unwrap_or(user_host);
        let path = &url[colon + 1..];
        // Reconstruct as "host/path" — need owned string
        return Some(format!(
            "{}/{}",
            host,
            path.strip_suffix(".git").unwrap_or(path).trim_matches('/')
        ))
        .filter(|s| s.len() > host.len() + 1);
    } else {
        None
    }?;

    let full = full.strip_suffix(".git").unwrap_or(full).trim_matches('/');

    if full.is_empty() || !full.contains('/') {
        None
    } else {
        Some(full.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_path_from_url() {
        assert_eq!(
            repo_path_from_url("https://github.com/acme/widgets"),
            Some("github.com/acme/widgets".into())
        );
        assert_eq!(
            repo_path_from_url("https://github.com/acme/widgets.git"),
            Some("github.com/acme/widgets".into())
        );
        assert_eq!(
            repo_path_from_url("git@github.com:org/repo.git"),
            Some("github.com/org/repo".into())
        );
        assert_eq!(
            repo_path_from_url("https://gitlab.com/team/project"),
            Some("gitlab.com/team/project".into())
        );
        assert_eq!(repo_path_from_url(""), None);
        // Host-only URL should return None
        assert_eq!(repo_path_from_url("https://github.com"), None);
    }
}
