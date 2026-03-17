//! Scheduled re-indexing of the codebase knowledge graph.
//!
//! Periodically runs the analyzer pipeline (scan → api-surface → sync) and
//! posts observations when new dependency cycles are detected.  The last
//! indexed commit hash is persisted so unchanged codebases are skipped.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::project::api::ProjectsApi;

// ---------------------------------------------------------------------------
// Persisted state — last indexed commit per project
// ---------------------------------------------------------------------------

const STATE_FILE: &str = ".analyzer-last-indexed.json";

#[derive(Debug, Serialize, Deserialize, Default)]
struct IndexerState {
    /// Map of project_id → last indexed commit hash.
    projects: HashMap<String, ProjectIndexState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectIndexState {
    commit_hash: String,
    /// Known dependency cycles (module-level) from last run.
    #[serde(default)]
    known_cycles: Vec<Vec<String>>,
}

impl IndexerState {
    fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pipeline runner
// ---------------------------------------------------------------------------

/// Get the HEAD commit hash for a git repo.
fn get_head_commit(git_root: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(git_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to run git rev-parse HEAD")?;

    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if any source files changed between two commits.
fn has_source_changes(git_root: &Path, old_commit: &str, new_commit: &str) -> Result<bool> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            "--name-only",
            old_commit,
            new_commit,
            "--",
            "*.rs",
            "*.ts",
            "*.tsx",
            "*.sql",
        ])
        .current_dir(git_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to run git diff --name-only")?;

    if !output.status.success() {
        // If the old commit doesn't exist (first run), treat as changed
        return Ok(true);
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    Ok(!diff.trim().is_empty())
}

/// Run the analyzer scan step, writing output to the given path.
fn run_scan(git_root: &Path, output_path: &Path) {
    diraigent_analyzer::scan::run(
        git_root.to_path_buf(),
        Some(output_path.to_path_buf()),
        false,
    );
}

/// Run the analyzer api-surface step, writing output to the given path.
fn run_api_surface(git_root: &Path, output_path: &Path) {
    diraigent_analyzer::api_surface::run(
        git_root.to_path_buf(),
        Some(output_path.to_path_buf()),
        "json",
        false,
    );
}

/// Run the full indexing pipeline for a single project.
///
/// Returns the list of newly detected cycles (empty if none).
async fn run_pipeline(
    api: &ProjectsApi,
    project_id: &str,
    git_root: &Path,
    state: &mut IndexerState,
    state_path: &Path,
) -> Result<Vec<Vec<String>>> {
    let head_commit = get_head_commit(git_root)?;

    // Check if we already indexed this commit
    if let Some(prev) = state.projects.get(project_id) {
        if prev.commit_hash == head_commit {
            debug!(
                "project {project_id}: already indexed commit {}",
                &head_commit[..12]
            );
            return Ok(Vec::new());
        }

        // Check if source files actually changed
        if !has_source_changes(git_root, &prev.commit_hash, &head_commit)? {
            debug!(
                "project {project_id}: no source file changes between {} and {}",
                &prev.commit_hash[..12],
                &head_commit[..12]
            );
            // Update commit hash but keep known cycles
            state.projects.insert(
                project_id.to_string(),
                ProjectIndexState {
                    commit_hash: head_commit,
                    known_cycles: prev.known_cycles.clone(),
                },
            );
            state.save(state_path)?;
            return Ok(Vec::new());
        }
    }

    info!(
        "project {project_id}: indexing commit {}",
        &head_commit[..12]
    );

    // Create a temp dir for intermediate files
    let tmp_dir = std::env::temp_dir().join(format!("diraigent-index-{project_id}"));
    std::fs::create_dir_all(&tmp_dir)?;

    let scan_path = tmp_dir.join("scan.json");
    let api_surface_path = tmp_dir.join("api-surface.json");

    // Step 1: Scan
    run_scan(git_root, &scan_path);
    info!("project {project_id}: scan complete");

    // Step 2: API Surface
    run_api_surface(git_root, &api_surface_path);
    info!("project {project_id}: api-surface complete");

    // Step 3: Sync to knowledge store
    let cache_path = tmp_dir.join(".analyzer-sync-cache.json");

    // Copy the persistent sync cache into temp dir if it exists
    let persistent_cache = git_root.join(".analyzer-sync-cache.json");
    if persistent_cache.exists() {
        std::fs::copy(&persistent_cache, &cache_path).ok();
    }

    let sync_config = diraigent_analyzer::sync::SyncConfig {
        manifest_path: scan_path.clone(),
        summaries_path: None,
        api_surface_path: api_surface_path.clone(),
        cache_path: cache_path.clone(),
        api_url: api.base_url().to_string(),
        api_token: api.api_token().to_string(),
        project_id: project_id.to_string(),
        agent_id: Some(api.agent_id().to_string()),
        dry_run: false,
    };
    if let Err(e) = diraigent_analyzer::sync::run(sync_config).await {
        anyhow::bail!("analyzer sync failed: {e}");
    }
    info!("project {project_id}: sync complete");

    // Copy sync cache back to persistent location
    if cache_path.exists() {
        std::fs::copy(&cache_path, &persistent_cache).ok();
    }

    // Step 4: Detect cycles
    let scan_data = std::fs::read_to_string(&scan_path)?;
    let manifest: diraigent_analyzer::scan::Manifest = serde_json::from_str(&scan_data)?;
    let graph = diraigent_analyzer::graph::build_module_graph(&manifest);
    let current_cycles = diraigent_analyzer::graph::detect_module_cycles(&graph);

    // Find NEW cycles (not previously known)
    let prev_cycles: HashSet<Vec<String>> = state
        .projects
        .get(project_id)
        .map(|s| s.known_cycles.iter().cloned().collect())
        .unwrap_or_default();

    let new_cycles: Vec<Vec<String>> = current_cycles
        .iter()
        .filter(|c| !prev_cycles.contains(*c))
        .cloned()
        .collect();

    // File observations for new cycles
    for cycle in &new_cycles {
        let cycle_str = cycle.join(" → ");
        let body = serde_json::json!({
            "kind": "risk",
            "title": format!("New dependency cycle detected: {cycle_str}"),
            "description": format!(
                "The dependency graph now contains a cycle between modules: {cycle_str}. \
                 Circular dependencies make the codebase harder to reason about, \
                 test in isolation, and refactor. Consider breaking the cycle by \
                 extracting shared types into a common module."
            ),
            "severity": "medium"
        });
        if let Err(e) = api.post_observation(project_id, &body).await {
            warn!("project {project_id}: failed to post cycle observation: {e}");
        } else {
            info!("project {project_id}: filed observation for cycle: {cycle_str}");
        }
    }

    // Update state
    state.projects.insert(
        project_id.to_string(),
        ProjectIndexState {
            commit_hash: head_commit,
            known_cycles: current_cycles.clone(),
        },
    );
    state.save(state_path)?;

    // Cleanup temp files (keep sync cache)
    std::fs::remove_file(&scan_path).ok();
    std::fs::remove_file(&api_surface_path).ok();

    if new_cycles.is_empty() && !current_cycles.is_empty() {
        info!(
            "project {project_id}: indexing complete, {} known cycles (no new ones)",
            current_cycles.len()
        );
    } else if !new_cycles.is_empty() {
        warn!(
            "project {project_id}: indexing complete, {} NEW cycles detected",
            new_cycles.len()
        );
    } else {
        info!("project {project_id}: indexing complete, no cycles");
    }

    Ok(new_cycles)
}

// ---------------------------------------------------------------------------
// Public API — called from the main loop
// ---------------------------------------------------------------------------

/// Run the indexer for all projects visible to this agent.
///
/// Skips projects with `git_mode == "none"` and projects whose HEAD hasn't
/// changed since the last run.
pub async fn tick(api: &ProjectsApi, projects_path: &Path) {
    let state_path = projects_path.join(STATE_FILE);
    let mut state = IndexerState::load(&state_path);

    let projects = match api.list_projects().await {
        Ok(p) => p,
        Err(e) => {
            warn!("indexer: failed to list projects: {e}");
            return;
        }
    };

    for project in &projects {
        let project_id = match project["id"].as_str() {
            Some(id) => id,
            None => continue,
        };

        let git_mode = project["git_mode"].as_str().unwrap_or("standalone");
        if git_mode == "none" {
            continue;
        }

        // Resolve git root
        let git_root_rel = project["git_root"].as_str().unwrap_or("");
        let slug = project["slug"].as_str().unwrap_or("");

        let git_root = if !git_root_rel.is_empty() {
            projects_path.join(git_root_rel)
        } else if !slug.is_empty() {
            projects_path.join(slug)
        } else {
            continue;
        };

        if !git_root.join(".git").exists() && !git_root.exists() {
            debug!(
                "indexer: skipping {project_id} — git root not found: {}",
                git_root.display()
            );
            continue;
        }

        match run_pipeline(api, project_id, &git_root, &mut state, &state_path).await {
            Ok(new_cycles) => {
                if !new_cycles.is_empty() {
                    info!(
                        "indexer: project {project_id} — {} new cycle(s) detected",
                        new_cycles.len()
                    );
                }
            }
            Err(e) => {
                warn!("indexer: project {project_id} — pipeline failed: {e:#}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use diraigent_analyzer::graph::{build_module_graph, detect_module_cycles};
    use diraigent_analyzer::scan::{FileEntry, Manifest, Stats};
    use std::collections::{BTreeMap, BTreeSet};

    fn make_file(path: &str, language: &str, imports: Vec<&str>) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            language: language.to_string(),
            imports: imports.into_iter().map(String::from).collect(),
            exports: Vec::new(),
            routes: Vec::new(),
            docstring: None,
        }
    }

    fn make_manifest(files: Vec<FileEntry>) -> Manifest {
        Manifest {
            stats: Stats {
                total_files: files.len(),
                by_language: std::collections::HashMap::new(),
                elapsed_ms: 0,
            },
            files,
        }
    }

    #[test]
    fn test_detect_cycles_no_cycle() {
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), BTreeSet::from(["B".to_string()]));
        graph.insert("B".to_string(), BTreeSet::from(["C".to_string()]));
        graph.insert("C".to_string(), BTreeSet::new());

        let cycles = detect_module_cycles(&graph);
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_detect_cycles_simple_cycle() {
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), BTreeSet::from(["B".to_string()]));
        graph.insert("B".to_string(), BTreeSet::from(["A".to_string()]));

        let cycles = detect_module_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert!(cycles[0].contains(&"A".to_string()));
        assert!(cycles[0].contains(&"B".to_string()));
    }

    #[test]
    fn test_detect_cycles_three_node_cycle() {
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), BTreeSet::from(["B".to_string()]));
        graph.insert("B".to_string(), BTreeSet::from(["C".to_string()]));
        graph.insert("C".to_string(), BTreeSet::from(["A".to_string()]));

        let cycles = detect_module_cycles(&graph);
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);
    }

    #[test]
    fn test_detect_cycles_multiple_independent() {
        let mut graph = BTreeMap::new();
        graph.insert("A".to_string(), BTreeSet::from(["B".to_string()]));
        graph.insert("B".to_string(), BTreeSet::from(["A".to_string()]));
        graph.insert("C".to_string(), BTreeSet::from(["D".to_string()]));
        graph.insert("D".to_string(), BTreeSet::from(["C".to_string()]));

        let cycles = detect_module_cycles(&graph);
        assert_eq!(cycles.len(), 2);
    }

    #[test]
    fn test_detect_cycles_deterministic() {
        let mut graph = BTreeMap::new();
        graph.insert("X".to_string(), BTreeSet::from(["Y".to_string()]));
        graph.insert("Y".to_string(), BTreeSet::from(["X".to_string()]));

        let c1 = detect_module_cycles(&graph);
        let c2 = detect_module_cycles(&graph);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_build_module_graph() {
        let manifest = make_manifest(vec![
            make_file("apps/api/src/main.rs", "rust", vec!["shared_utils::config"]),
            make_file(
                "libs/common-rust/shared-utils/src/config.rs",
                "rust",
                vec![],
            ),
        ]);

        let graph = build_module_graph(&manifest);
        // apps/api should depend on libs/common-rust (via shared_utils)
        assert!(graph.contains_key("apps/api"));
    }

    #[test]
    fn test_state_roundtrip() {
        let dir = std::env::temp_dir().join("indexer_test_state");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");

        let mut state = IndexerState::default();
        state.projects.insert(
            "proj-1".to_string(),
            ProjectIndexState {
                commit_hash: "abc123".to_string(),
                known_cycles: vec![vec!["A".to_string(), "B".to_string()]],
            },
        );

        state.save(&path).unwrap();
        let loaded = IndexerState::load(&path);
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects["proj-1"].commit_hash, "abc123");
        assert_eq!(loaded.projects["proj-1"].known_cycles.len(), 1);

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_state_load_nonexistent() {
        let state = IndexerState::load(Path::new("/nonexistent/state.json"));
        assert!(state.projects.is_empty());
    }
}
