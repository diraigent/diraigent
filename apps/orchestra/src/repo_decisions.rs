//! Discovers and loads decision YAML files from `.diraigent/decisions/` in a project repo.
//!
//! Repo-based decisions let teams version-control Architecture Decision Records (ADRs)
//! alongside code. Each `.yaml` file in the directory defines one decision.
//! Optional `*_file` fields allow loading long-form content from separate markdown files.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Valid decision statuses.
const VALID_STATUSES: &[&str] = &[
    "proposed",
    "accepted",
    "rejected",
    "superseded",
    "deprecated",
];

/// An alternative considered in a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub name: String,
    #[serde(default)]
    pub pros: Option<String>,
    #[serde(default)]
    pub cons: Option<String>,
}

/// A decision definition parsed from a repo YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDecision {
    /// Human-readable decision title.
    pub title: String,
    /// Decision status: proposed, accepted, rejected, superseded, deprecated.
    #[serde(default = "default_status")]
    pub status: String,
    /// Context describing the problem or situation.
    #[serde(default)]
    pub context: String,
    /// Path to a markdown file containing the context (overrides `context`).
    #[serde(default)]
    pub context_file: Option<String>,
    /// The decision that was made.
    #[serde(default)]
    pub decision: Option<String>,
    /// Path to a markdown file containing the decision (overrides `decision`).
    #[serde(default)]
    pub decision_file: Option<String>,
    /// Rationale for the decision.
    #[serde(default)]
    pub rationale: Option<String>,
    /// Path to a markdown file containing the rationale (overrides `rationale`).
    #[serde(default)]
    pub rationale_file: Option<String>,
    /// Alternatives that were considered.
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
    /// Consequences of the decision.
    #[serde(default)]
    pub consequences: Option<String>,
    /// Path to a markdown file containing the consequences (overrides `consequences`).
    #[serde(default)]
    pub consequences_file: Option<String>,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_status() -> String {
    "proposed".to_string()
}

/// Scan `.diraigent/decisions/` for `*.yaml` files.
///
/// Returns sorted list of discovered file paths. Returns an empty vec (not an error)
/// if the directory does not exist.
pub fn discover_decisions(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let dir = repo_root.join(".diraigent").join("decisions");
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

/// Parse a single YAML decision file into a [`RepoDecision`].
///
/// Performs validation:
/// - `title` must be non-empty.
/// - `status` must be one of the valid statuses.
/// - `context` must be non-empty (either inline or from file).
///
/// After parsing, resolves any `*_file` references by reading the referenced files.
pub fn parse_decision(path: &Path) -> Result<RepoDecision> {
    let display_path = path.display().to_string();

    let content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {display_path}"))?;

    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("invalid YAML in {display_path}"))?;

    let json_value: serde_json::Value = serde_json::to_value(&yaml_value)
        .with_context(|| format!("YAML->JSON conversion failed for {display_path}"))?;

    let mut decision: RepoDecision = serde_json::from_value(json_value)
        .with_context(|| format!("failed to parse decision fields in {display_path}"))?;

    // --- Validation ---

    if decision.title.trim().is_empty() {
        bail!("{display_path}: \"title\" must not be empty");
    }

    if !VALID_STATUSES.contains(&decision.status.as_str()) {
        bail!(
            "{display_path}: invalid status \"{}\" (must be one of: {})",
            decision.status,
            VALID_STATUSES.join(", ")
        );
    }

    // Resolve file references before validating context content
    if let Some(parent) = path.parent() {
        resolve_file_references(&mut decision, parent);
    }

    if decision.context.trim().is_empty() {
        bail!("{display_path}: \"context\" must not be empty (provide inline or via context_file)");
    }

    Ok(decision)
}

/// Resolve `*_file` references in a decision.
///
/// For each field that has a corresponding `*_file` variant, reads the referenced file
/// (relative to `base_dir`) and sets the field content to the file content.
/// The `*_file` field is then cleared.
///
/// Supported file references:
/// - `context_file` -> `context`
/// - `decision_file` -> `decision`
/// - `rationale_file` -> `rationale`
/// - `consequences_file` -> `consequences`
///
/// If both the field and `*_file` are present, `*_file` takes precedence.
/// Failures (missing file, read errors) are logged as warnings and the field
/// is left unchanged (falls back to inline content if present).
pub fn resolve_file_references(decision: &mut RepoDecision, base_dir: &Path) {
    if let Some(content) = try_read_file_ref("context", &decision.context_file, base_dir) {
        decision.context = content;
        decision.context_file = None;
    }

    if let Some(content) = try_read_file_ref("decision", &decision.decision_file, base_dir) {
        decision.decision = Some(content);
        decision.decision_file = None;
    }

    if let Some(content) = try_read_file_ref("rationale", &decision.rationale_file, base_dir) {
        decision.rationale = Some(content);
        decision.rationale_file = None;
    }

    if let Some(content) = try_read_file_ref("consequences", &decision.consequences_file, base_dir)
    {
        decision.consequences = Some(content);
        decision.consequences_file = None;
    }
}

/// Try to read a file reference. Returns `Some(content)` on success, `None` on failure or
/// if no file reference is set.
fn try_read_file_ref(
    field_name: &str,
    file_ref: &Option<String>,
    base_dir: &Path,
) -> Option<String> {
    let file_path_str = match file_ref.as_deref() {
        Some(f) if !f.is_empty() => f,
        _ => return None,
    };

    let file_path = base_dir.join(file_path_str);
    match std::fs::read_to_string(&file_path) {
        Ok(content) => {
            info!("Decision: loaded {field_name} from {file_path_str}");
            Some(content)
        }
        Err(e) => {
            warn!(
                "Decision: failed to read {field_name}_file '{}' (resolved to {}): {e}",
                file_path_str,
                file_path.display()
            );
            None
        }
    }
}

/// Load all decisions from a repo's `.diraigent/decisions/` directory.
///
/// This is a convenience wrapper around [`discover_decisions`] + [`parse_decision`].
/// Files that fail to parse are logged as warnings and skipped.
pub fn load_repo_decisions(repo_root: &Path) -> Result<Vec<(String, RepoDecision)>> {
    let paths = discover_decisions(repo_root)?;
    let mut decisions = Vec::with_capacity(paths.len());

    for path in &paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        match parse_decision(path) {
            Ok(d) => decisions.push((name, d)),
            Err(e) => {
                warn!("Skipping invalid decision {}: {e:#}", path.display());
            }
        }
    }

    Ok(decisions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_yaml(dir: &Path, name: &str, content: &str) {
        fs::create_dir_all(dir.join(".diraigent/decisions")).unwrap();
        fs::write(dir.join(format!(".diraigent/decisions/{name}")), content).unwrap();
    }

    #[test]
    fn discover_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = discover_decisions(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn discover_finds_yaml_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_yaml(
            tmp.path(),
            "adr-001.yaml",
            "title: ADR 001\ncontext: some context\nstatus: proposed\n",
        );
        write_yaml(
            tmp.path(),
            "adr-002.yml",
            "title: ADR 002\ncontext: more context\nstatus: accepted\n",
        );
        // non-yaml file should be ignored
        fs::write(tmp.path().join(".diraigent/decisions/readme.md"), "# hi").unwrap();

        let result = discover_decisions(tmp.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_valid_decision() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Use PostgreSQL for persistence"
status: accepted
context: "We need a relational database for our data"
decision: "Use PostgreSQL 15+"
rationale: "Strong JSON support, mature ecosystem"
alternatives:
  - name: "MySQL"
    pros: "Widely known"
    cons: "Weaker JSON support"
consequences: "Need to manage PG backups"
tags: ["architecture", "database"]
"#;
        write_yaml(tmp.path(), "use-postgres.yaml", yaml);

        let path = tmp.path().join(".diraigent/decisions/use-postgres.yaml");
        let d = parse_decision(&path).unwrap();
        assert_eq!(d.title, "Use PostgreSQL for persistence");
        assert_eq!(d.status, "accepted");
        assert_eq!(d.context, "We need a relational database for our data");
        assert_eq!(d.decision.as_deref(), Some("Use PostgreSQL 15+"));
        assert_eq!(d.alternatives.len(), 1);
        assert_eq!(d.alternatives[0].name, "MySQL");
        assert_eq!(d.tags, vec!["architecture", "database"]);
    }

    #[test]
    fn parse_defaults_status_to_proposed() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Some Decision"
context: "We need to decide something"
"#;
        write_yaml(tmp.path(), "default-status.yaml", yaml);

        let path = tmp.path().join(".diraigent/decisions/default-status.yaml");
        let d = parse_decision(&path).unwrap();
        assert_eq!(d.status, "proposed");
    }

    #[test]
    fn parse_rejects_empty_title() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "title: \"\"\ncontext: some context\nstatus: proposed\n";
        write_yaml(tmp.path(), "bad.yaml", yaml);

        let path = tmp.path().join(".diraigent/decisions/bad.yaml");
        let err = parse_decision(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("must not be empty"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_invalid_status() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Bad Status"
context: "some context"
status: active
"#;
        write_yaml(tmp.path(), "badstatus.yaml", yaml);

        let path = tmp.path().join(".diraigent/decisions/badstatus.yaml");
        let err = parse_decision(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid status"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_empty_context() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "title: \"No Context\"\nstatus: proposed\n";
        write_yaml(tmp.path(), "nocontext.yaml", yaml);

        let path = tmp.path().join(".diraigent/decisions/nocontext.yaml");
        let err = parse_decision(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("context"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_resolves_context_file() {
        let tmp = tempfile::tempdir().unwrap();
        let decisions_dir = tmp.path().join(".diraigent/decisions");
        fs::create_dir_all(&decisions_dir).unwrap();

        fs::write(
            decisions_dir.join("context.md"),
            "# Context\n\nDetailed context from file.\n",
        )
        .unwrap();

        let yaml = r#"
title: "With File Ref"
status: proposed
context_file: context.md
"#;
        fs::write(decisions_dir.join("with-file.yaml"), yaml).unwrap();

        let path = decisions_dir.join("with-file.yaml");
        let d = parse_decision(&path).unwrap();
        assert_eq!(d.context, "# Context\n\nDetailed context from file.\n");
        assert!(d.context_file.is_none());
    }

    #[test]
    fn parse_file_ref_overrides_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let decisions_dir = tmp.path().join(".diraigent/decisions");
        fs::create_dir_all(&decisions_dir).unwrap();

        fs::write(decisions_dir.join("rationale.md"), "From file\n").unwrap();

        let yaml = r#"
title: "Override Test"
status: accepted
context: "some context"
rationale: "Inline rationale that should be overridden"
rationale_file: rationale.md
"#;
        fs::write(decisions_dir.join("override.yaml"), yaml).unwrap();

        let path = decisions_dir.join("override.yaml");
        let d = parse_decision(&path).unwrap();
        assert_eq!(d.rationale.as_deref(), Some("From file\n"));
    }

    #[test]
    fn parse_file_ref_missing_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let decisions_dir = tmp.path().join(".diraigent/decisions");
        fs::create_dir_all(&decisions_dir).unwrap();

        let yaml = r#"
title: "Missing File"
status: proposed
context: "Fallback context"
decision: "Fallback decision"
decision_file: nonexistent.md
"#;
        fs::write(decisions_dir.join("fallback.yaml"), yaml).unwrap();

        let path = decisions_dir.join("fallback.yaml");
        let d = parse_decision(&path).unwrap();
        // File not found, so inline decision should remain
        assert_eq!(d.decision.as_deref(), Some("Fallback decision"));
        // decision_file should still be present since resolution failed
        assert!(d.decision_file.is_some());
    }

    #[test]
    fn load_skips_invalid_files() {
        let tmp = tempfile::tempdir().unwrap();
        // valid
        let good = r#"
title: "Good Decision"
context: "some context"
status: accepted
"#;
        write_yaml(tmp.path(), "good.yaml", good);
        // invalid (empty title)
        write_yaml(
            tmp.path(),
            "bad.yaml",
            "title: \"\"\ncontext: ctx\nstatus: proposed\n",
        );

        let decisions = load_repo_decisions(tmp.path()).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].1.title, "Good Decision");
    }
}
