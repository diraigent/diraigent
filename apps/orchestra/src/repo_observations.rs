//! Discovers and loads observation YAML files from `.diraigent/observations/` in a project repo.
//!
//! Repo-based observations let teams version-control known issues, tech debt items,
//! and improvement ideas alongside code. Each `.yaml` file in the directory defines one observation.
//! Optional `description_file` field allows loading long-form content from a separate markdown file.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Valid observation kinds.
const VALID_KINDS: &[&str] = &[
    "insight",
    "risk",
    "opportunity",
    "smell",
    "inconsistency",
    "improvement",
];

/// Valid observation severities.
const VALID_SEVERITIES: &[&str] = &["info", "low", "medium", "high", "critical"];

/// An observation definition parsed from a repo YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoObservation {
    /// Human-readable observation title.
    pub title: String,
    /// Observation kind: insight, risk, opportunity, smell, inconsistency, improvement.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Observation severity: info, low, medium, high, critical.
    #[serde(default = "default_severity")]
    pub severity: String,
    /// Description of the observation (inline).
    #[serde(default)]
    pub description: String,
    /// Path to a markdown file containing the description (overrides `description`).
    #[serde(default)]
    pub description_file: Option<String>,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_kind() -> String {
    "insight".to_string()
}

fn default_severity() -> String {
    "info".to_string()
}

/// Scan `.diraigent/observations/` for `*.yaml` files.
///
/// Returns sorted list of discovered file paths. Returns an empty vec (not an error)
/// if the directory does not exist.
pub fn discover_observations(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let dir = repo_root.join(".diraigent").join("observations");
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

/// Parse a single YAML observation file into a [`RepoObservation`].
///
/// Performs validation:
/// - `title` must be non-empty.
/// - `kind` must be one of the valid kinds.
/// - `severity` must be one of the valid severities.
///
/// After parsing, resolves any `description_file` reference by reading the referenced file.
pub fn parse_observation(path: &Path) -> Result<RepoObservation> {
    let display_path = path.display().to_string();

    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {display_path}"))?;

    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("invalid YAML in {display_path}"))?;

    let json_value: serde_json::Value = serde_json::to_value(&yaml_value)
        .with_context(|| format!("YAML->JSON conversion failed for {display_path}"))?;

    let mut observation: RepoObservation = serde_json::from_value(json_value)
        .with_context(|| format!("failed to parse observation fields in {display_path}"))?;

    // --- Validation ---

    if observation.title.trim().is_empty() {
        bail!("{display_path}: \"title\" must not be empty");
    }

    if !VALID_KINDS.contains(&observation.kind.as_str()) {
        bail!(
            "{display_path}: invalid kind \"{}\" (must be one of: {})",
            observation.kind,
            VALID_KINDS.join(", ")
        );
    }

    if !VALID_SEVERITIES.contains(&observation.severity.as_str()) {
        bail!(
            "{display_path}: invalid severity \"{}\" (must be one of: {})",
            observation.severity,
            VALID_SEVERITIES.join(", ")
        );
    }

    // Resolve description_file reference before validating description content
    if let Some(parent) = path.parent() {
        resolve_file_references(&mut observation, parent);
    }

    Ok(observation)
}

/// Resolve `description_file` reference in an observation.
///
/// If `description_file` is set, reads the referenced file (relative to `base_dir`)
/// and sets `description` to the file content. The `description_file` field is then cleared.
///
/// If both `description` and `description_file` are present, `description_file` takes precedence.
/// Failures (missing file, read errors) are logged as warnings and the field
/// is left unchanged (falls back to inline description if present).
pub fn resolve_file_references(observation: &mut RepoObservation, base_dir: &Path) {
    let file_path_str = match observation.description_file.as_deref() {
        Some(f) if !f.is_empty() => f,
        _ => return,
    };

    let file_path = base_dir.join(file_path_str);
    match std::fs::read_to_string(&file_path) {
        Ok(content) => {
            info!("Observation: loaded description from {file_path_str}");
            observation.description = content;
            observation.description_file = None;
        }
        Err(e) => {
            warn!(
                "Observation: failed to read description_file '{}' (resolved to {}): {e}",
                file_path_str,
                file_path.display()
            );
        }
    }
}

/// Load all observations from a repo's `.diraigent/observations/` directory.
///
/// This is a convenience wrapper around [`discover_observations`] + [`parse_observation`].
/// Files that fail to parse are logged as warnings and skipped.
pub fn load_repo_observations(repo_root: &Path) -> Result<Vec<(String, RepoObservation)>> {
    let paths = discover_observations(repo_root)?;
    let mut entries = Vec::with_capacity(paths.len());

    for path in &paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        match parse_observation(path) {
            Ok(o) => entries.push((name, o)),
            Err(e) => {
                warn!("Skipping invalid observation {}: {e:#}", path.display());
            }
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_yaml(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir.join(".diraigent/observations")).unwrap();
        fs::write(dir.join(format!(".diraigent/observations/{name}")), content).unwrap();
    }

    #[test]
    fn discover_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = discover_observations(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn discover_finds_yaml_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_yaml(
            tmp.path(),
            "tech-debt.yaml",
            "title: Tech Debt\nkind: smell\nseverity: medium\ndescription: some desc\n",
        );
        write_yaml(
            tmp.path(),
            "risk-item.yml",
            "title: Risk\nkind: risk\nseverity: high\ndescription: risk desc\n",
        );
        // non-yaml file should be ignored
        fs::write(tmp.path().join(".diraigent/observations/readme.md"), "# hi").unwrap();

        let result = discover_observations(tmp.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_valid_observation() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Legacy auth middleware needs refactoring"
kind: smell
severity: medium
description: |
  The auth middleware in src/auth.rs uses a deprecated pattern.
tags: ["tech-debt", "auth"]
"#;
        write_yaml(tmp.path(), "auth-refactor.yaml", yaml);

        let path = tmp
            .path()
            .join(".diraigent/observations/auth-refactor.yaml");
        let o = parse_observation(&path).unwrap();
        assert_eq!(o.title, "Legacy auth middleware needs refactoring");
        assert_eq!(o.kind, "smell");
        assert_eq!(o.severity, "medium");
        assert!(o.description.contains("deprecated pattern"));
        assert_eq!(o.tags, vec!["tech-debt", "auth"]);
    }

    #[test]
    fn parse_defaults_kind_to_insight() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Some Observation"
description: "Some description"
"#;
        write_yaml(tmp.path(), "default-kind.yaml", yaml);

        let path = tmp.path().join(".diraigent/observations/default-kind.yaml");
        let o = parse_observation(&path).unwrap();
        assert_eq!(o.kind, "insight");
    }

    #[test]
    fn parse_defaults_severity_to_info() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Some Observation"
description: "Some description"
"#;
        write_yaml(tmp.path(), "default-sev.yaml", yaml);

        let path = tmp.path().join(".diraigent/observations/default-sev.yaml");
        let o = parse_observation(&path).unwrap();
        assert_eq!(o.severity, "info");
    }

    #[test]
    fn parse_rejects_empty_title() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "title: \"\"\nkind: smell\nseverity: low\ndescription: some desc\n";
        write_yaml(tmp.path(), "bad.yaml", yaml);

        let path = tmp.path().join(".diraigent/observations/bad.yaml");
        let err = parse_observation(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("must not be empty"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_invalid_kind() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Bad Kind"
kind: invalid_kind
severity: low
description: "some desc"
"#;
        write_yaml(tmp.path(), "badkind.yaml", yaml);

        let path = tmp.path().join(".diraigent/observations/badkind.yaml");
        let err = parse_observation(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid kind"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_invalid_severity() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Bad Severity"
kind: smell
severity: extreme
description: "some desc"
"#;
        write_yaml(tmp.path(), "badsev.yaml", yaml);

        let path = tmp.path().join(".diraigent/observations/badsev.yaml");
        let err = parse_observation(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid severity"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_resolves_description_file() {
        let tmp = tempfile::tempdir().unwrap();
        let obs_dir = tmp.path().join(".diraigent/observations");
        fs::create_dir_all(&obs_dir).unwrap();

        fs::write(
            obs_dir.join("details.md"),
            "# Tech Debt\n\nDetailed description from file.\n",
        )
        .unwrap();

        let yaml = r#"
title: "With File Ref"
kind: smell
severity: low
description_file: details.md
"#;
        fs::write(obs_dir.join("with-file.yaml"), yaml).unwrap();

        let path = obs_dir.join("with-file.yaml");
        let o = parse_observation(&path).unwrap();
        assert_eq!(
            o.description,
            "# Tech Debt\n\nDetailed description from file.\n"
        );
        assert!(o.description_file.is_none());
    }

    #[test]
    fn parse_description_file_overrides_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let obs_dir = tmp.path().join(".diraigent/observations");
        fs::create_dir_all(&obs_dir).unwrap();

        fs::write(obs_dir.join("desc.md"), "From file\n").unwrap();

        let yaml = r#"
title: "Override Test"
kind: risk
severity: high
description: "Inline description that should be overridden"
description_file: desc.md
"#;
        fs::write(obs_dir.join("override.yaml"), yaml).unwrap();

        let path = obs_dir.join("override.yaml");
        let o = parse_observation(&path).unwrap();
        assert_eq!(o.description, "From file\n");
    }

    #[test]
    fn parse_description_file_missing_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let obs_dir = tmp.path().join(".diraigent/observations");
        fs::create_dir_all(&obs_dir).unwrap();

        let yaml = r#"
title: "Missing File"
kind: improvement
severity: low
description: "Fallback description"
description_file: nonexistent.md
"#;
        fs::write(obs_dir.join("fallback.yaml"), yaml).unwrap();

        let path = obs_dir.join("fallback.yaml");
        let o = parse_observation(&path).unwrap();
        // File not found, so inline description should remain
        assert_eq!(o.description, "Fallback description");
        // description_file should still be present since resolution failed
        assert!(o.description_file.is_some());
    }

    #[test]
    fn load_skips_invalid_files() {
        let tmp = tempfile::tempdir().unwrap();
        // valid
        let good = r#"
title: "Good Observation"
kind: smell
severity: medium
description: "some description"
"#;
        write_yaml(tmp.path(), "good.yaml", good);
        // invalid (empty title)
        write_yaml(
            tmp.path(),
            "bad.yaml",
            "title: \"\"\nkind: smell\nseverity: low\ndescription: desc\n",
        );

        let entries = load_repo_observations(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1.title, "Good Observation");
    }
}
