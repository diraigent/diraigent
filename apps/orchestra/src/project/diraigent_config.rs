//! Reads `.diraigent/config.toml` and manages hook scripts in managed repos.
//!
//! ## Layout
//!
//! ```text
//! .diraigent/
//! ├── config.toml       # per-hook template selection
//! └── release.sh        # release script (template-managed or custom)
//! ```
//!
//! When `config.toml` sets `template = "squash-merge"` under `[release]`, the
//! orchestra regenerates `release.sh` from the built-in template before each
//! run.  When `template` is absent the script is treated as user-owned.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Top-level `.diraigent/config.toml` structure.
#[derive(Debug, Default, Deserialize)]
pub struct DiraigentConfig {
    #[serde(default)]
    pub release: HookConfig,
    #[serde(default)]
    pub deploy: HookConfig,
    #[serde(default)]
    pub test: HookConfig,
    #[serde(default)]
    pub notify: HookConfig,
}

/// Per-hook configuration.
#[derive(Debug, Default, Deserialize)]
pub struct HookConfig {
    /// Built-in template name.  When set the script is regenerated on each run.
    /// When absent (or null) the script is treated as custom / user-owned.
    pub template: Option<String>,
    /// Extra environment variables passed to the script.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Environment variables passed by orchestra to every `.diraigent/` script.
pub struct ScriptEnv {
    pub project_id: String,
    pub project_path: PathBuf,
    pub branch: String,
    pub target_branch: String,
    pub user_id: String,
    pub version: String,
}

impl ScriptEnv {
    /// Convert to a list of (key, value) pairs for `Command::envs`.
    pub fn as_pairs(&self) -> Vec<(&str, String)> {
        vec![
            ("DIRAIGENT_PROJECT_ID", self.project_id.clone()),
            (
                "DIRAIGENT_PROJECT_PATH",
                self.project_path.to_string_lossy().to_string(),
            ),
            ("DIRAIGENT_BRANCH", self.branch.clone()),
            ("DIRAIGENT_TARGET_BRANCH", self.target_branch.clone()),
            ("DIRAIGENT_USER_ID", self.user_id.clone()),
            ("DIRAIGENT_VERSION", self.version.clone()),
        ]
    }
}

// ── Built-in templates ──────────────────────────────────────────────

/// Registry of built-in release templates.
pub fn release_template(name: &str) -> Option<&'static str> {
    match name {
        "squash-merge" => Some(include_str!("../templates/release_squash_merge.sh")),
        "merge-commit" => Some(include_str!("../templates/release_merge_commit.sh")),
        "tag-only" => Some(include_str!("../templates/release_tag_only.sh")),
        _ => None,
    }
}

// ── Config loading ──────────────────────────────────────────────────

/// Load `.diraigent/config.toml` from a repo root.  Returns `Default` when
/// the file does not exist.
pub fn load_config(repo_root: &Path) -> Result<DiraigentConfig> {
    let config_path = repo_root.join(".diraigent/config.toml");
    if !config_path.exists() {
        debug!(
            path = %config_path.display(),
            ".diraigent/config.toml not found, using defaults"
        );
        return Ok(DiraigentConfig::default());
    }

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let config: DiraigentConfig =
        toml::from_str(&content).with_context(|| format!("parse {}", config_path.display()))?;

    debug!(path = %config_path.display(), "loaded .diraigent/config.toml");
    Ok(config)
}

/// Ensure the `.diraigent/` directory exists, regenerate the script from a
/// template if one is configured, and return the script path.
///
/// Returns `None` when no script exists and no template is configured.
pub fn prepare_hook_script(
    repo_root: &Path,
    hook_name: &str,
    hook_config: &HookConfig,
) -> Result<Option<PathBuf>> {
    let dir = repo_root.join(".diraigent");
    let script_path = dir.join(format!("{hook_name}.sh"));

    if let Some(ref tpl_name) = hook_config.template {
        // Template mode — regenerate the script.
        let tpl_fn = match hook_name {
            "release" => release_template,
            // Future: deploy_template, test_template, etc.
            _ => return Ok(None),
        };
        let tpl_content = tpl_fn(tpl_name)
            .with_context(|| format!("unknown {hook_name} template: {tpl_name}"))?;

        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        std::fs::write(&script_path, tpl_content)
            .with_context(|| format!("write {}", script_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                .with_context(|| format!("chmod {}", script_path.display()))?;
        }

        info!(
            hook = hook_name,
            template = tpl_name,
            "regenerated .diraigent/{hook_name}.sh from template"
        );
        return Ok(Some(script_path));
    }

    // Custom mode — use existing script if present.
    if script_path.exists() {
        debug!(hook = hook_name, "using custom .diraigent/{hook_name}.sh");
        Ok(Some(script_path))
    } else {
        Ok(None)
    }
}
