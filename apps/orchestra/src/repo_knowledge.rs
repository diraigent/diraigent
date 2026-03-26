//! Discovers and loads knowledge YAML files from `.diraigent/knowledge/` in a project repo.
//!
//! Repo-based knowledge lets teams version-control architecture docs, conventions, and patterns
//! alongside code. Each `.yaml` file in the directory defines one knowledge entry.
//! Optional `content_file` field allows loading long-form content from a separate markdown file.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Valid knowledge categories.
const VALID_CATEGORIES: &[&str] = &[
    "architecture",
    "convention",
    "pattern",
    "anti_pattern",
    "setup",
    "general",
    "reference",
];

/// A knowledge entry parsed from a repo YAML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoKnowledge {
    /// Human-readable knowledge title.
    pub title: String,
    /// Knowledge category.
    #[serde(default = "default_category")]
    pub category: String,
    /// The knowledge content (inline).
    #[serde(default)]
    pub content: String,
    /// Path to a markdown file containing the content (overrides `content`).
    #[serde(default)]
    pub content_file: Option<String>,
    /// Tags for categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_category() -> String {
    "general".to_string()
}

/// Scan `.diraigent/knowledge/` for `*.yaml` files.
///
/// Returns sorted list of discovered file paths. Returns an empty vec (not an error)
/// if the directory does not exist.
pub fn discover_knowledge(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let dir = repo_root.join(".diraigent").join("knowledge");
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

/// Parse a single YAML knowledge file into a [`RepoKnowledge`].
///
/// Performs validation:
/// - `title` must be non-empty.
/// - `category` must be one of the valid categories.
/// - `content` must be non-empty (either inline or from file).
///
/// After parsing, resolves any `content_file` reference by reading the referenced file.
pub fn parse_knowledge(path: &Path) -> Result<RepoKnowledge> {
    let display_path = path.display().to_string();

    let raw_content =
        std::fs::read_to_string(path).with_context(|| format!("failed to read {display_path}"))?;

    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&raw_content)
        .with_context(|| format!("invalid YAML in {display_path}"))?;

    let json_value: serde_json::Value = serde_json::to_value(&yaml_value)
        .with_context(|| format!("YAML->JSON conversion failed for {display_path}"))?;

    let mut knowledge: RepoKnowledge = serde_json::from_value(json_value)
        .with_context(|| format!("failed to parse knowledge fields in {display_path}"))?;

    // --- Validation ---

    if knowledge.title.trim().is_empty() {
        bail!("{display_path}: \"title\" must not be empty");
    }

    if !VALID_CATEGORIES.contains(&knowledge.category.as_str()) {
        bail!(
            "{display_path}: invalid category \"{}\" (must be one of: {})",
            knowledge.category,
            VALID_CATEGORIES.join(", ")
        );
    }

    // Resolve content_file reference before validating content
    if let Some(parent) = path.parent() {
        resolve_file_references(&mut knowledge, parent);
    }

    if knowledge.content.trim().is_empty() {
        bail!("{display_path}: \"content\" must not be empty (provide inline or via content_file)");
    }

    Ok(knowledge)
}

/// Resolve `content_file` reference in a knowledge entry.
///
/// If `content_file` is set, reads the referenced file (relative to `base_dir`)
/// and sets `content` to the file content. The `content_file` field is then cleared.
///
/// If both `content` and `content_file` are present, `content_file` takes precedence.
/// Failures (missing file, read errors) are logged as warnings and the field
/// is left unchanged (falls back to inline content if present).
pub fn resolve_file_references(knowledge: &mut RepoKnowledge, base_dir: &Path) {
    let file_path_str = match knowledge.content_file.as_deref() {
        Some(f) if !f.is_empty() => f,
        _ => return,
    };

    let file_path = base_dir.join(file_path_str);
    match std::fs::read_to_string(&file_path) {
        Ok(content) => {
            info!("Knowledge: loaded content from {file_path_str}");
            knowledge.content = content;
            knowledge.content_file = None;
        }
        Err(e) => {
            warn!(
                "Knowledge: failed to read content_file '{}' (resolved to {}): {e}",
                file_path_str,
                file_path.display()
            );
        }
    }
}

/// Load all knowledge entries from a repo's `.diraigent/knowledge/` directory.
///
/// This is a convenience wrapper around [`discover_knowledge`] + [`parse_knowledge`].
/// Files that fail to parse are logged as warnings and skipped.
pub fn load_repo_knowledge(repo_root: &Path) -> Result<Vec<(String, RepoKnowledge)>> {
    let paths = discover_knowledge(repo_root)?;
    let mut entries = Vec::with_capacity(paths.len());

    for path in &paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        match parse_knowledge(path) {
            Ok(k) => entries.push((name, k)),
            Err(e) => {
                warn!("Skipping invalid knowledge {}: {e:#}", path.display());
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
        fs::create_dir_all(dir.join(".diraigent/knowledge")).unwrap();
        fs::write(dir.join(format!(".diraigent/knowledge/{name}")), content).unwrap();
    }

    #[test]
    fn discover_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = discover_knowledge(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn discover_finds_yaml_files() {
        let tmp = tempfile::tempdir().unwrap();
        write_yaml(
            tmp.path(),
            "api-auth.yaml",
            "title: API Auth\ncategory: pattern\ncontent: some content\n",
        );
        write_yaml(
            tmp.path(),
            "conventions.yml",
            "title: Conventions\ncategory: convention\ncontent: more content\n",
        );
        // non-yaml file should be ignored
        fs::write(tmp.path().join(".diraigent/knowledge/readme.md"), "# hi").unwrap();

        let result = discover_knowledge(tmp.path()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_valid_knowledge() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "API Authentication Pattern"
category: pattern
content: |
  All API endpoints require Bearer token auth.
  Use the Authorization header.
tags: ["api", "security"]
"#;
        write_yaml(tmp.path(), "api-auth.yaml", yaml);

        let path = tmp.path().join(".diraigent/knowledge/api-auth.yaml");
        let k = parse_knowledge(&path).unwrap();
        assert_eq!(k.title, "API Authentication Pattern");
        assert_eq!(k.category, "pattern");
        assert!(k.content.contains("Bearer token auth"));
        assert_eq!(k.tags, vec!["api", "security"]);
    }

    #[test]
    fn parse_defaults_category_to_general() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Some Knowledge"
content: "Some content here"
"#;
        write_yaml(tmp.path(), "default-cat.yaml", yaml);

        let path = tmp.path().join(".diraigent/knowledge/default-cat.yaml");
        let k = parse_knowledge(&path).unwrap();
        assert_eq!(k.category, "general");
    }

    #[test]
    fn parse_rejects_empty_title() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "title: \"\"\ncategory: pattern\ncontent: some content\n";
        write_yaml(tmp.path(), "bad.yaml", yaml);

        let path = tmp.path().join(".diraigent/knowledge/bad.yaml");
        let err = parse_knowledge(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("must not be empty"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_invalid_category() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
title: "Bad Category"
category: invalid_cat
content: "some content"
"#;
        write_yaml(tmp.path(), "badcat.yaml", yaml);

        let path = tmp.path().join(".diraigent/knowledge/badcat.yaml");
        let err = parse_knowledge(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid category"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_rejects_empty_content() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = "title: \"No Content\"\ncategory: pattern\n";
        write_yaml(tmp.path(), "nocontent.yaml", yaml);

        let path = tmp.path().join(".diraigent/knowledge/nocontent.yaml");
        let err = parse_knowledge(&path).unwrap_err();
        assert!(
            format!("{err:#}").contains("content"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn parse_resolves_content_file() {
        let tmp = tempfile::tempdir().unwrap();
        let knowledge_dir = tmp.path().join(".diraigent/knowledge");
        fs::create_dir_all(&knowledge_dir).unwrap();

        fs::write(
            knowledge_dir.join("auth-pattern.md"),
            "# Auth Pattern\n\nDetailed content from file.\n",
        )
        .unwrap();

        let yaml = r#"
title: "With File Ref"
category: pattern
content_file: auth-pattern.md
"#;
        fs::write(knowledge_dir.join("with-file.yaml"), yaml).unwrap();

        let path = knowledge_dir.join("with-file.yaml");
        let k = parse_knowledge(&path).unwrap();
        assert_eq!(k.content, "# Auth Pattern\n\nDetailed content from file.\n");
        assert!(k.content_file.is_none());
    }

    #[test]
    fn parse_content_file_overrides_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let knowledge_dir = tmp.path().join(".diraigent/knowledge");
        fs::create_dir_all(&knowledge_dir).unwrap();

        fs::write(knowledge_dir.join("content.md"), "From file\n").unwrap();

        let yaml = r#"
title: "Override Test"
category: convention
content: "Inline content that should be overridden"
content_file: content.md
"#;
        fs::write(knowledge_dir.join("override.yaml"), yaml).unwrap();

        let path = knowledge_dir.join("override.yaml");
        let k = parse_knowledge(&path).unwrap();
        assert_eq!(k.content, "From file\n");
    }

    #[test]
    fn parse_content_file_missing_falls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let knowledge_dir = tmp.path().join(".diraigent/knowledge");
        fs::create_dir_all(&knowledge_dir).unwrap();

        let yaml = r#"
title: "Missing File"
category: pattern
content: "Fallback content"
content_file: nonexistent.md
"#;
        fs::write(knowledge_dir.join("fallback.yaml"), yaml).unwrap();

        let path = knowledge_dir.join("fallback.yaml");
        let k = parse_knowledge(&path).unwrap();
        // File not found, so inline content should remain
        assert_eq!(k.content, "Fallback content");
        // content_file should still be present since resolution failed
        assert!(k.content_file.is_some());
    }

    #[test]
    fn load_skips_invalid_files() {
        let tmp = tempfile::tempdir().unwrap();
        // valid
        let good = r#"
title: "Good Knowledge"
category: pattern
content: "some content"
"#;
        write_yaml(tmp.path(), "good.yaml", good);
        // invalid (empty title)
        write_yaml(
            tmp.path(),
            "bad.yaml",
            "title: \"\"\ncategory: pattern\ncontent: ctx\n",
        );

        let entries = load_repo_knowledge(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].1.title, "Good Knowledge");
    }
}
