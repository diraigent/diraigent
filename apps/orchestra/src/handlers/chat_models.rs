use anyhow::{Context, bail};
use diraigent_types::ChatModelCatalog;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::project::api::ProjectsApi;

type CatalogCache = HashMap<(Uuid, PathBuf), (Instant, Vec<String>)>;
static CACHE: LazyLock<Mutex<CatalogCache>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn discover(
    api: &ProjectsApi,
    project_id: Uuid,
    projects_path: &Path,
    refresh: bool,
) -> anyhow::Result<ChatModelCatalog> {
    let id = project_id.to_string();
    // Subsequent chat must resolve the same fresh project configuration.
    super::chat::invalidate_project_info(&id).await;
    let project = api.get_project(&id).await?;
    let provider = project["metadata"]["chat_provider"]
        .as_str()
        .filter(|v| !v.is_empty())
        .unwrap_or("opencode")
        .to_owned();
    let default_model = project["metadata"]["chat_model"]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(str::to_owned);
    if provider != "opencode" {
        return Ok(ChatModelCatalog {
            models: if provider == "claude-code" {
                vec!["sonnet".into(), "opus".into(), "haiku".into()]
            } else {
                vec![]
            },
            provider,
            default_model,
        });
    }
    let working_dir = crate::project::paths::resolve_working_dir(api, &id, projects_path).await?;
    let key = (project_id, working_dir.clone());
    let cached = if refresh {
        None
    } else {
        CACHE
            .lock()
            .await
            .get(&key)
            .filter(|(time, _)| time.elapsed() < Duration::from_secs(60))
            .map(|(_, models)| models.clone())
    };
    let models = match cached {
        Some(models) => models,
        None => {
            let models = list_models(Path::new("opencode"), &working_dir).await?;
            let mut cache = CACHE.lock().await;
            cache.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(60));
            cache.insert(key, (Instant::now(), models.clone()));
            models
        }
    };
    Ok(ChatModelCatalog {
        provider,
        default_model: default_model.filter(|v| valid_model_id(v)).or_else(|| {
            std::env::var("OPENCODE_MODEL")
                .ok()
                .filter(|v| valid_model_id(v))
        }),
        models,
    })
}

/// V1 and V2 both support `models`. V2's background catalog may need a moment
/// to populate on the first call. Its CLI selects the project from cwd.
async fn list_models(binary: &Path, working_dir: &Path) -> anyhow::Result<Vec<String>> {
    list_models_with_timeout(binary, working_dir, Duration::from_secs(25)).await
}

async fn list_models_with_timeout(
    binary: &Path,
    working_dir: &Path,
    timeout: Duration,
) -> anyhow::Result<Vec<String>> {
    tokio::time::timeout(timeout, async {
        for attempt in 0..3 {
            let mut child = Command::new(binary)
                .arg("models")
                .current_dir(working_dir)
                .env("NO_COLOR", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .context("Run OpenCode models")?;
            let mut output = Vec::new();
            child
                .stdout
                .take()
                .context("Missing catalog output")?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut output)
                .await?;
            if output.len() > 1024 * 1024 {
                bail!("Model catalog too large");
            }
            if !child.wait().await?.success() {
                bail!("OpenCode models failed");
            }
            let models = parse_models(&output)?;
            if !models.is_empty() {
                return Ok(models);
            }
            if attempt < 2 {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        bail!("OpenCode returned no available models")
    })
    .await
    .context("OpenCode model discovery timed out")?
}

fn valid_model_id(id: &str) -> bool {
    id.len() <= 256
        && id
            .split_once('/')
            .is_some_and(|(provider, model)| !provider.is_empty() && !model.is_empty())
        && !id.contains("//")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_.:-#".contains(&b))
}

fn parse_models(output: &[u8]) -> anyhow::Result<Vec<String>> {
    if output.len() > 1024 * 1024 {
        bail!("Model catalog too large");
    }
    let text = std::str::from_utf8(output).context("Invalid model catalog encoding")?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| valid_model_id(line))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_only_contains_sorted_qualified_ids() {
        let models = parse_models(b"github-copilot/gpt-5.3-codex\r\nollama/qwen3.5:9b\ngithub-copilot/gpt-5.3-codex\nLoading catalog...\n{\"token\":\"synthetic\"}\nhttps://provider.example/model\n/provider\nprovider/\n").unwrap();
        assert_eq!(
            models,
            vec!["github-copilot/gpt-5.3-codex", "ollama/qwen3.5:9b"]
        );
    }

    #[test]
    fn rejects_invalid_encoding_and_oversized_output() {
        assert!(parse_models(&[255]).is_err());
        assert!(parse_models(&vec![b'a'; 1024 * 1024 + 1]).is_err());
        assert!(valid_model_id("openrouter/qwen/qwen3#high"));
        assert!(!valid_model_id("sonnet"));
    }

    #[tokio::test]
    async fn discovery_uses_project_directory_and_handles_cli_failure() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("fake-opencode");
        std::fs::write(&cli, "#!/bin/sh\n[ \"$1\" = models ] || exit 3\n[ -f project-config ] || exit 4\nprintf 'custom/project-model\\n'\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(list_models(&cli, dir.path()).await.is_err());
        std::fs::write(dir.path().join("project-config"), "").unwrap();
        assert_eq!(
            list_models(&cli, dir.path()).await.unwrap(),
            vec!["custom/project-model"]
        );
    }

    #[tokio::test]
    async fn discovery_times_out_without_waiting_for_a_stalled_cli() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("fake-opencode");
        std::fs::write(&cli, "#!/bin/sh\nexec sleep 5\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = list_models_with_timeout(&cli, dir.path(), Duration::from_millis(50)).await;
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn discovery_retries_an_initially_empty_catalog() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("fake-opencode");
        std::fs::write(&cli, "#!/bin/sh\nif [ ! -f warmed ]; then touch warmed; exit 0; fi\nprintf 'custom/warmed-model\\n'\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            list_models(&cli, dir.path()).await.unwrap(),
            vec!["custom/warmed-model"]
        );
    }
}
