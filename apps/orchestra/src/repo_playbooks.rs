//! Discovers and loads playbook YAML files from `.diraigent/playbooks/` in a project repo.
//!
//! Repo-based playbooks let teams version-control their playbook definitions alongside code.
//! Each `.yaml` file in the directory defines one playbook. The filename stem (e.g. `standard`
//! from `standard.yaml`) becomes the playbook identifier.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

/// A playbook definition parsed from a repo YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoPlaybook {
    /// Filename stem (e.g. "standard" from standard.yaml). Derived from filename if omitted.
    #[serde(default)]
    pub name: String,
    /// Human-readable playbook title.
    pub title: String,
    /// Short description of the step flow (e.g. "implement → review").
    #[serde(default)]
    pub trigger_description: Option<String>,
    /// Starting state: "ready" or "backlog". Defaults to "ready".
    #[serde(default = "default_initial_state")]
    pub initial_state: String,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Arbitrary metadata (e.g. `{ "git_strategy": "merge_to_default" }`).
    #[serde(default = "default_metadata")]
    pub metadata: serde_json::Value,
    /// Array of step objects. Must be non-empty; each step must have a `name` field.
    pub steps: serde_json::Value,
}

fn default_initial_state() -> String {
    "ready".to_string()
}

fn default_metadata() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// Scan `.diraigent/playbooks/` for `*.yaml` files.
///
/// Returns sorted list of discovered file paths. Returns an empty vec (not an error)
/// if the directory does not exist.
pub fn discover_playbooks(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let dir = repo_root.join(".diraigent").join("playbooks");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .with_context(|| format!("failed to read directory {}", dir.display()))?;

    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read entry in {}", dir.display()))?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

/// Parse a single YAML playbook file into a [`RepoPlaybook`].
///
/// Performs validation:
/// - `steps` must be a non-empty array.
/// - Each step must have a `name` field (string).
/// - `initial_state` must be `"ready"` or `"backlog"` (defaults to `"ready"` if omitted).
pub fn parse_playbook(path: &Path) -> Result<RepoPlaybook> {
    let display_path = path.display().to_string();

    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {display_path}"))?;

    // Parse YAML into a generic Value first, then convert to JSON-compatible Value.
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("invalid YAML in {display_path}"))?;

    // Convert YAML Value → JSON Value for uniform handling.
    let json_value: serde_json::Value = serde_json::to_value(&yaml_value)
        .with_context(|| format!("YAML→JSON conversion failed for {display_path}"))?;

    // Deserialize the JSON Value into our struct.
    let mut playbook: RepoPlaybook = serde_json::from_value(json_value)
        .with_context(|| format!("failed to parse playbook fields in {display_path}"))?;

    // Derive `name` from filename stem if not overridden by the YAML.
    if playbook.name.is_empty() {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            playbook.name = stem.to_string();
        } else {
            bail!("{display_path}: unable to derive playbook name from filename");
        }
    }

    // --- Validation ---

    // initial_state must be "ready" or "backlog".
    match playbook.initial_state.as_str() {
        "ready" | "backlog" => {}
        other => bail!(
            "{display_path}: invalid initial_state \"{other}\" (must be \"ready\" or \"backlog\")"
        ),
    }

    // steps must be a non-empty array.
    let steps_arr = playbook
        .steps
        .as_array()
        .with_context(|| format!("{display_path}: \"steps\" must be an array"))?;

    if steps_arr.is_empty() {
        bail!("{display_path}: \"steps\" array must not be empty");
    }

    // Each step must have a `name` field (string).
    for (i, step) in steps_arr.iter().enumerate() {
        match step.get("name").and_then(|v| v.as_str()) {
            Some(_) => {}
            None => {
                bail!("{display_path}: step[{i}] is missing a \"name\" field (must be a string)")
            }
        }
    }

    Ok(playbook)
}

/// Load all playbooks from a repo's `.diraigent/playbooks/` directory.
///
/// This is a convenience wrapper around [`discover_playbooks`] + [`parse_playbook`].
/// Files that fail to parse are logged as warnings and skipped — they do not prevent
/// other playbooks from loading.
pub fn load_repo_playbooks(repo_root: &Path) -> Result<Vec<RepoPlaybook>> {
    let paths = discover_playbooks(repo_root)?;
    let mut playbooks = Vec::with_capacity(paths.len());

    for path in &paths {
        match parse_playbook(path) {
            Ok(pb) => playbooks.push(pb),
            Err(e) => {
                warn!("Skipping invalid playbook {}: {e:#}", path.display());
            }
        }
    }

    Ok(playbooks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_yaml(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir.join(".diraigent/playbooks")).unwrap();
        fs::write(dir.join(format!(".diraigent/playbooks/{name}")), content).unwrap();
    }

    #[test]
    fn discover_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = discover_playbooks(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn discover_finds_yaml_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_yaml(tmp.path(), "a.yaml", "title: A\nsteps: []\n");
        write_yaml(tmp.path(), "b.yml", "title: B\nsteps: []\n");
        // non-yaml file should be ignored
        fs::write(tmp.path().join(".diraigent/playbooks/readme.md"), "# hi").unwrap();

        let result = discover_playbooks(tmp.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_valid_playbook() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: Standard Lifecycle
trigger_description: "implement → review"
initial_state: ready
tags: [default]
metadata:
  git_strategy: merge_to_default
steps:
  - name: implement
    budget: 12.0
    allowed_tools: full
    context_level: full
    description: "Do the work"
  - name: review
    model: claude-sonnet-4-6
    budget: 5.0
    allowed_tools: readonly
    description: "Review the work"
"#;
        write_yaml(tmp.path(), "standard.yaml", yaml);

        let path = tmp.path().join(".diraigent/playbooks/standard.yaml");
        let pb = parse_playbook(&path).unwrap();
        assert_eq!(pb.title, "Standard Lifecycle");
        assert_eq!(pb.initial_state, "ready");
        assert_eq!(pb.tags, vec!["default"]);
        assert_eq!(pb.steps.as_array().unwrap().len(), 2);
    }

    #[test]
    fn parse_defaults_name_from_filename() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: My Playbook
steps:
  - name: implement
    description: "Do stuff"
"#;
        write_yaml(tmp.path(), "custom.yaml", yaml);

        let path = tmp.path().join(".diraigent/playbooks/custom.yaml");
        let pb = parse_playbook(&path).unwrap();
        // name was empty in YAML, so derived from filename
        assert_eq!(pb.name, "custom");
        assert_eq!(pb.initial_state, "ready"); // default
    }

    #[test]
    fn parse_rejects_empty_steps() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "title: Bad\nsteps: []\n";
        write_yaml(tmp.path(), "bad.yaml", yaml);

        let path = tmp.path().join(".diraigent/playbooks/bad.yaml");
        let err = parse_playbook(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("must not be empty"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_step_without_name() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: Bad
steps:
  - description: "missing name"
"#;
        write_yaml(tmp.path(), "noname.yaml", yaml);

        let path = tmp.path().join(".diraigent/playbooks/noname.yaml");
        let err = parse_playbook(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("missing a \"name\" field"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_invalid_initial_state() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: Bad
initial_state: running
steps:
  - name: implement
"#;
        write_yaml(tmp.path(), "badstate.yaml", yaml);

        let path = tmp.path().join(".diraigent/playbooks/badstate.yaml");
        let err = parse_playbook(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid initial_state"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn load_skips_invalid_files() {
        let tmp = tempfile::tempdir().unwrap();
        // valid
        let good = r#"
title: Good
steps:
  - name: implement
"#;
        write_yaml(tmp.path(), "good.yaml", good);
        // invalid (empty steps)
        write_yaml(tmp.path(), "bad.yaml", "title: Bad\nsteps: []\n");

        let playbooks = load_repo_playbooks(tmp.path()).unwrap();
        assert_eq!(playbooks.len(), 1);
        assert_eq!(playbooks[0].title, "Good");
    }
}
