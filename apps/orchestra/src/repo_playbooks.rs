//! Discovers and loads playbook YAML files from `.diraigent/playbooks/` in a project repo.
//!
//! Repo-based playbooks let teams version-control their playbook definitions alongside code.
//! Each `.yaml` file in the directory defines one playbook. The filename stem (e.g. `standard`
//! from `standard.yaml`) becomes the playbook identifier.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

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
///
/// After parsing, resolves any `description_file` references in steps by reading
/// the referenced files and setting the `description` field.
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

    // Resolve description_file references in steps.
    // The base directory for relative paths is the directory containing the YAML file.
    if let Some(parent) = path.parent() {
        resolve_description_files(&mut playbook.steps, parent);
    }

    Ok(playbook)
}

/// Resolve `description_file` references in playbook steps.
///
/// For each step that has a `description_file` field, reads the referenced file
/// (relative to `base_dir`) and sets the step's `description` to the file content.
/// The `description_file` field is then removed from the step JSON.
///
/// This allows step descriptions to be authored as standalone markdown files,
/// giving compatibility with standard prompt workflows:
///
/// ```yaml
/// steps:
///   - name: implement
///     description_file: steps/implement.md
///     budget: 12.0
/// ```
///
/// If both `description` and `description_file` are present, `description_file`
/// takes precedence and `description` is overwritten.
///
/// Failures (missing file, read errors) are logged as warnings and the step
/// is left unchanged (falls back to inline `description` if present).
pub fn resolve_description_files(steps: &mut serde_json::Value, base_dir: &Path) {
    let steps_arr = match steps.as_array_mut() {
        Some(arr) => arr,
        None => return,
    };

    for step in steps_arr.iter_mut() {
        let obj = match step.as_object_mut() {
            Some(o) => o,
            None => continue,
        };

        let file_ref = match obj.get("description_file").and_then(|v| v.as_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };

        // Extract step_name as owned String to avoid borrow conflicts.
        let step_name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let had_description = obj.contains_key("description");

        let file_path = base_dir.join(&file_ref);
        match std::fs::read_to_string(&file_path) {
            Ok(content) => {
                if had_description {
                    info!(
                        "Step '{}': description_file overrides inline description (file: {})",
                        step_name, file_ref
                    );
                }
                obj.insert(
                    "description".to_string(),
                    serde_json::Value::String(content),
                );
                obj.remove("description_file");
                info!("Step '{}': loaded description from {}", step_name, file_ref);
            }
            Err(e) => {
                warn!(
                    "Step '{}': failed to read description_file '{}' (resolved to {}): {e}",
                    step_name,
                    file_ref,
                    file_path.display()
                );
                // Leave step as-is; inline description (if any) will be used.
            }
        }
    }
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

/// Slugify a playbook title for matching against repo filenames.
///
/// Converts "Standard Lifecycle" → "standard-lifecycle", "My Playbook" → "my-playbook".
/// Strips non-alphanumeric characters and replaces spaces/underscores with hyphens.
pub fn slugify_title(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if (ch == ' ' || ch == '_' || ch == '-') && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// Try to find a matching repo playbook for an API playbook.
///
/// Matching strategy:
/// 1. Exact match: API playbook title slugified matches a repo playbook name
/// 2. Prefix match: repo playbook name is a prefix of the slugified API title
///    (e.g. "standard" matches "Standard Lifecycle" → "standard-lifecycle")
/// 3. API playbook metadata `repo_name` matches repo playbook name
///
/// Returns the matching [`RepoPlaybook`] if found, or `None`.
pub fn find_repo_playbook_for_api(
    repo_playbooks: &[RepoPlaybook],
    api_title: &str,
    api_metadata: Option<&serde_json::Value>,
) -> Option<RepoPlaybook> {
    if repo_playbooks.is_empty() {
        return None;
    }

    let slug = slugify_title(api_title);

    // 1. Exact match on slugified title
    if let Some(pb) = repo_playbooks.iter().find(|pb| pb.name == slug) {
        return Some(pb.clone());
    }

    // 2. Prefix match: repo name is a prefix of the slugified title
    //    e.g. "standard" matches "standard-lifecycle"
    if let Some(pb) = repo_playbooks.iter().find(|pb| slug.starts_with(&pb.name)) {
        return Some(pb.clone());
    }

    // 3. API metadata repo_name match
    if let Some(meta) = api_metadata
        && let Some(repo_name) = meta["repo_name"].as_str()
        && let Some(pb) = repo_playbooks.iter().find(|pb| pb.name == repo_name)
    {
        return Some(pb.clone());
    }

    None
}

/// Load repo playbooks and try to find one matching the given API playbook.
///
/// This is a convenience function that combines `load_repo_playbooks` + `find_repo_playbook_for_api`.
/// Returns `None` if:
/// - The repo_root has no `.diraigent/playbooks/` directory
/// - No playbooks match
/// - Any I/O error occurs (logged as warning)
pub fn find_repo_override(
    repo_root: &Path,
    api_title: &str,
    api_metadata: Option<&serde_json::Value>,
) -> Option<RepoPlaybook> {
    match load_repo_playbooks(repo_root) {
        Ok(playbooks) => find_repo_playbook_for_api(&playbooks, api_title, api_metadata),
        Err(e) => {
            warn!(
                "repo_playbooks: failed to load from {}: {e:#}",
                repo_root.display()
            );
            None
        }
    }
}

/// Find a playbook by name from the repo's `.diraigent/playbooks/` directory.
pub fn find_playbook_by_name(repo_root: &Path, name: &str) -> Option<RepoPlaybook> {
    let path = repo_root
        .join(".diraigent")
        .join("playbooks")
        .join(format!("{name}.yaml"));
    if path.exists() {
        parse_playbook(&path).ok()
    } else {
        None
    }
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

    #[test]
    fn slugify_title_basic() {
        assert_eq!(slugify_title("Standard Lifecycle"), "standard-lifecycle");
        assert_eq!(slugify_title("My_Playbook"), "my-playbook");
        assert_eq!(slugify_title("simple"), "simple");
        assert_eq!(slugify_title("  Spaces  "), "spaces");
    }

    #[test]
    fn find_repo_playbook_exact_match() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: Standard Lifecycle
steps:
  - name: implement
"#;
        write_yaml(tmp.path(), "standard-lifecycle.yaml", yaml);
        let playbooks = load_repo_playbooks(tmp.path()).unwrap();

        let found = find_repo_playbook_for_api(&playbooks, "Standard Lifecycle", None);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "standard-lifecycle");
    }

    #[test]
    fn find_repo_playbook_prefix_match() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: Standard
steps:
  - name: implement
"#;
        write_yaml(tmp.path(), "standard.yaml", yaml);
        let playbooks = load_repo_playbooks(tmp.path()).unwrap();

        // "standard" should match "Standard Lifecycle" via prefix
        let found = find_repo_playbook_for_api(&playbooks, "Standard Lifecycle", None);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "standard");
    }

    #[test]
    fn find_repo_playbook_no_match() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: Custom
steps:
  - name: implement
"#;
        write_yaml(tmp.path(), "custom.yaml", yaml);
        let playbooks = load_repo_playbooks(tmp.path()).unwrap();

        let found = find_repo_playbook_for_api(&playbooks, "Standard Lifecycle", None);
        assert!(found.is_none());
    }

    #[test]
    fn parse_resolves_description_file() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join(".diraigent/playbooks");
        fs::create_dir_all(playbooks_dir.join("steps")).unwrap();

        // Write a markdown description file
        fs::write(
            playbooks_dir.join("steps/implement.md"),
            "## Your Job\n\nDo the implementation work.\n",
        )
        .unwrap();

        let yaml = r#"
title: With Markdown
steps:
  - name: implement
    description_file: steps/implement.md
    budget: 12.0
"#;
        fs::write(playbooks_dir.join("with-md.yaml"), yaml).unwrap();

        let path = playbooks_dir.join("with-md.yaml");
        let pb = parse_playbook(&path).unwrap();
        let steps = pb.steps.as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0]["description"].as_str().unwrap(),
            "## Your Job\n\nDo the implementation work.\n"
        );
        // description_file should be removed after resolution
        assert!(steps[0].get("description_file").is_none());
    }

    #[test]
    fn parse_description_file_overrides_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join(".diraigent/playbooks");
        fs::create_dir_all(&playbooks_dir).unwrap();

        // Write a markdown description file
        fs::write(playbooks_dir.join("review.md"), "Review from file\n").unwrap();

        let yaml = r#"
title: Override Test
steps:
  - name: review
    description: "Inline description that should be overridden"
    description_file: review.md
    budget: 5.0
"#;
        fs::write(playbooks_dir.join("override.yaml"), yaml).unwrap();

        let path = playbooks_dir.join("override.yaml");
        let pb = parse_playbook(&path).unwrap();
        let steps = pb.steps.as_array().unwrap();
        // description_file should win over inline description
        assert_eq!(
            steps[0]["description"].as_str().unwrap(),
            "Review from file\n"
        );
    }

    #[test]
    fn parse_description_file_missing_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let playbooks_dir = tmp.path().join(".diraigent/playbooks");
        fs::create_dir_all(&playbooks_dir).unwrap();

        let yaml = r#"
title: Missing File
steps:
  - name: implement
    description: "Fallback inline description"
    description_file: nonexistent.md
"#;
        fs::write(playbooks_dir.join("fallback.yaml"), yaml).unwrap();

        let path = playbooks_dir.join("fallback.yaml");
        let pb = parse_playbook(&path).unwrap();
        let steps = pb.steps.as_array().unwrap();
        // File not found, so inline description should remain
        assert_eq!(
            steps[0]["description"].as_str().unwrap(),
            "Fallback inline description"
        );
        // description_file should still be present since resolution failed
        assert!(steps[0].get("description_file").is_some());
    }

    #[test]
    fn resolve_description_files_no_steps() {
        // Should not panic when steps is not an array
        let mut steps = serde_json::json!("not an array");
        resolve_description_files(&mut steps, Path::new("/tmp"));
        assert_eq!(steps, serde_json::json!("not an array"));
    }

    #[test]
    fn resolve_description_files_mixed_steps() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("step1.md"), "Content from file").unwrap();

        let mut steps = serde_json::json!([
            { "name": "implement", "description_file": "step1.md" },
            { "name": "review", "description": "Inline only" }
        ]);

        resolve_description_files(&mut steps, tmp.path());

        let arr = steps.as_array().unwrap();
        // First step: description loaded from file
        assert_eq!(arr[0]["description"].as_str().unwrap(), "Content from file");
        assert!(arr[0].get("description_file").is_none());
        // Second step: unchanged
        assert_eq!(arr[1]["description"].as_str().unwrap(), "Inline only");
    }
}
