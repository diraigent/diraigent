//! Synchronize repository knowledge, decisions and observations.
use crate::engine::task_source::TaskSource;
use tracing::{info, warn};
pub async fn sync_project_decisions(api: &dyn TaskSource, repo_root: &std::path::Path) {
    let dir = repo_root.join(".diraigent").join("decisions");
    if !dir.is_dir() {
        return; // No decisions directory — nothing to do
    }

    // Discover YAML files
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("repository sync: failed to read {}: {e}", dir.display());
            return;
        }
    };

    let mut yaml_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            yaml_paths.push(path);
        }
    }
    yaml_paths.sort();

    if yaml_paths.is_empty() {
        return;
    }

    // Parse each YAML file into a RepoDecision
    let mut repo_decisions: Vec<(String, crate::repo_decisions::RepoDecision)> = Vec::new();
    for path in &yaml_paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        match crate::repo_decisions::parse_decision(path) {
            Ok(d) => repo_decisions.push((name, d)),
            Err(e) => warn!(
                "repository sync: failed to parse decision {}: {e:#}",
                path.display()
            ),
        }
    }

    if repo_decisions.is_empty() {
        return;
    }

    // Fetch project ID from the API using repo_root
    // We need the project_id to list decisions — derive it from the API context
    let project_id = match find_project_id_for_repo(api, repo_root).await {
        Some(id) => id,
        None => {
            warn!(
                "repository sync: cannot sync decisions — unable to determine project_id for repo"
            );
            return;
        }
    };

    // Fetch existing decisions from the API
    let existing = api.list_decisions(&project_id).await.unwrap_or_default();

    // Index existing repo-sourced decisions by their repo_file tag
    let mut repo_sourced: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for d in &existing {
        let tags = d["tags"].as_array();
        let is_repo = tags
            .map(|t| t.iter().any(|v| v.as_str() == Some("source:repo")))
            .unwrap_or(false);
        if is_repo
            && let Some(repo_file_tag) = tags.and_then(|t| {
                t.iter().find_map(|v| {
                    v.as_str()
                        .and_then(|s| s.strip_prefix("repo_file:"))
                        .map(|s| s.to_string())
                })
            })
        {
            repo_sourced.insert(repo_file_tag, d.clone());
        }
    }

    let mut created = 0u32;
    let mut updated = 0u32;
    let mut unchanged = 0u32;

    for (name, decision) in &repo_decisions {
        let repo_file = format!("{name}.yaml");

        // Build tags: keep user tags and add repo markers
        let mut tags = decision.tags.clone();
        if !tags.iter().any(|t| t == "source:repo") {
            tags.push("source:repo".to_string());
        }
        // Remove any existing repo_file: tag and add the current one
        tags.retain(|t: &String| !t.starts_with("repo_file:"));
        tags.push(format!("repo_file:{repo_file}"));

        // Build alternatives as JSON
        let alternatives: Vec<serde_json::Value> = decision
            .alternatives
            .iter()
            .map(|a| {
                let mut obj = serde_json::json!({"name": a.name});
                if let Some(ref pros) = a.pros {
                    obj["pros"] = serde_json::json!(pros);
                }
                if let Some(ref cons) = a.cons {
                    obj["cons"] = serde_json::json!(cons);
                }
                obj
            })
            .collect();

        if let Some(existing_d) = repo_sourced.remove(&repo_file) {
            // Check if content changed
            let content_changed = existing_d["title"].as_str().unwrap_or("") != decision.title
                || existing_d["status"].as_str().unwrap_or("proposed") != decision.status
                || existing_d["context"].as_str().unwrap_or("") != decision.context
                || existing_d["decision"].as_str() != decision.decision.as_deref()
                || existing_d["rationale"].as_str() != decision.rationale.as_deref()
                || existing_d["consequences"].as_str() != decision.consequences.as_deref();

            if content_changed {
                let decision_id = existing_d["id"].as_str().unwrap_or("");
                let body = serde_json::json!({
                    "title": decision.title,
                    "status": decision.status,
                    "context": decision.context,
                    "decision": decision.decision,
                    "rationale": decision.rationale,
                    "alternatives": alternatives,
                    "consequences": decision.consequences,
                    "tags": tags,
                });
                match api.update_decision(decision_id, &body).await {
                    Ok(_) => {
                        info!("repository sync: updated repo decision '{name}' (id={decision_id})");
                        updated += 1;
                    }
                    Err(e) => {
                        warn!("repository sync: failed to update repo decision '{name}': {e}")
                    }
                }
            } else {
                unchanged += 1;
            }
        } else {
            // Create new decision
            let body = serde_json::json!({
                "title": decision.title,
                "context": decision.context,
                "decision": decision.decision,
                "rationale": decision.rationale,
                "alternatives": alternatives,
                "consequences": decision.consequences,
                "tags": tags,
            });
            match api.post_decision(&project_id, &body).await {
                Ok(created_d) => {
                    // After creation, update with status and tags (CreateDecision doesn't have status)
                    if let Some(id) = created_d["id"].as_str() {
                        let update_body = serde_json::json!({
                            "status": decision.status,
                            "tags": tags,
                        });
                        if let Err(e) = api.update_decision(id, &update_body).await {
                            warn!(
                                "repository sync: created repo decision '{name}' but failed to set status/tags: {e}"
                            );
                        }
                    }
                    info!("repository sync: created repo decision '{name}'");
                    created += 1;
                }
                Err(e) => warn!("repository sync: failed to create repo decision '{name}': {e}"),
            }
        }
    }

    // Warn about orphaned repo-sourced decisions
    for (repo_file, d) in &repo_sourced {
        let title = d["title"].as_str().unwrap_or("unknown");
        warn!(
            "repository sync: API decision '{title}' (repo_file={repo_file}) \
             has no matching file in repo — consider removing it"
        );
    }

    let total = created + updated + unchanged;
    if total > 0 {
        info!(
            "repository sync: synced {total} repo decision(s) ({created} created, {updated} updated, {unchanged} unchanged)"
        );
    }
}

/// Sync repo-based knowledge entries to the API for a given project.
///
/// Discovers YAML knowledge files in `.diraigent/knowledge/` and syncs them to the API.
/// Uses `metadata.source = "repo"` and `metadata.repo_file` for identification.
/// Failures are non-fatal: errors are logged but do not prevent task execution.
pub async fn sync_project_knowledge(api: &dyn TaskSource, repo_root: &std::path::Path) {
    let dir = repo_root.join(".diraigent").join("knowledge");
    if !dir.is_dir() {
        return; // No knowledge directory — nothing to do
    }

    // Discover YAML files
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("repository sync: failed to read {}: {e}", dir.display());
            return;
        }
    };

    let mut yaml_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            yaml_paths.push(path);
        }
    }
    yaml_paths.sort();

    if yaml_paths.is_empty() {
        return;
    }

    // Parse each YAML file into a RepoKnowledge
    let mut repo_knowledge: Vec<(String, crate::repo_knowledge::RepoKnowledge)> = Vec::new();
    for path in &yaml_paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        match crate::repo_knowledge::parse_knowledge(path) {
            Ok(k) => repo_knowledge.push((name, k)),
            Err(e) => warn!(
                "repository sync: failed to parse knowledge {}: {e:#}",
                path.display()
            ),
        }
    }

    if repo_knowledge.is_empty() {
        return;
    }

    // Fetch project ID from the API using repo_root
    let project_id = match find_project_id_for_repo(api, repo_root).await {
        Some(id) => id,
        None => {
            warn!(
                "repository sync: cannot sync knowledge — unable to determine project_id for repo"
            );
            return;
        }
    };

    // Fetch existing knowledge from the API, filtered by source:repo tag
    let existing = api
        .list_knowledge(&project_id, Some("source:repo"), Some(500))
        .await
        .unwrap_or_default();

    // Index existing repo-sourced knowledge by their metadata.repo_file
    let mut repo_sourced: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for k in &existing {
        if k["metadata"]["source"].as_str() == Some("repo")
            && let Some(repo_file) = k["metadata"]["repo_file"].as_str()
        {
            repo_sourced.insert(repo_file.to_string(), k.clone());
        }
    }

    let mut created = 0u32;
    let mut updated = 0u32;
    let mut unchanged = 0u32;

    for (name, knowledge) in &repo_knowledge {
        let repo_file = format!("{name}.yaml");

        // Build tags: keep user tags and add repo marker
        let mut tags = knowledge.tags.clone();
        if !tags.iter().any(|t| t == "source:repo") {
            tags.push("source:repo".to_string());
        }

        // Build metadata with source=repo and repo_file markers
        let metadata = serde_json::json!({
            "source": "repo",
            "repo_file": repo_file,
        });

        if let Some(existing_k) = repo_sourced.remove(&repo_file) {
            // Check if content changed
            let content_changed = existing_k["title"].as_str().unwrap_or("") != knowledge.title
                || existing_k["category"].as_str().unwrap_or("general") != knowledge.category
                || existing_k["content"].as_str().unwrap_or("") != knowledge.content;

            if content_changed {
                let knowledge_id = existing_k["id"].as_str().unwrap_or("");
                let body = serde_json::json!({
                    "title": knowledge.title,
                    "category": knowledge.category,
                    "content": knowledge.content,
                    "tags": tags,
                    "metadata": metadata,
                });
                match api.update_knowledge(knowledge_id, &body).await {
                    Ok(_) => {
                        info!(
                            "repository sync: updated repo knowledge '{name}' (id={knowledge_id})"
                        );
                        updated += 1;
                    }
                    Err(e) => {
                        warn!("repository sync: failed to update repo knowledge '{name}': {e}");
                    }
                }
            } else {
                unchanged += 1;
            }
        } else {
            // Create new knowledge entry
            let body = serde_json::json!({
                "title": knowledge.title,
                "category": knowledge.category,
                "content": knowledge.content,
                "tags": tags,
                "metadata": metadata,
            });
            match api.post_knowledge(&project_id, &body).await {
                Ok(_) => {
                    info!("repository sync: created repo knowledge '{name}'");
                    created += 1;
                }
                Err(e) => warn!("repository sync: failed to create repo knowledge '{name}': {e}"),
            }
        }
    }

    // Warn about orphaned repo-sourced knowledge
    for (repo_file, k) in &repo_sourced {
        let title = k["title"].as_str().unwrap_or("unknown");
        warn!(
            "repository sync: API knowledge '{title}' (repo_file={repo_file}) \
             has no matching file in repo — consider removing it"
        );
    }

    let total = created + updated + unchanged;
    if total > 0 {
        info!(
            "repository sync: synced {total} repo knowledge entry/entries ({created} created, {updated} updated, {unchanged} unchanged)"
        );
    }
}

/// Sync repo-based observations to the API for a given project.
///
/// Discovers YAML observation files in `.diraigent/observations/` and syncs them to the API.
/// Uses `source = "repo"` and `metadata.repo_file` for identification.
/// Failures are non-fatal: errors are logged but do not prevent task execution.
pub async fn sync_project_observations(api: &dyn TaskSource, repo_root: &std::path::Path) {
    let dir = repo_root.join(".diraigent").join("observations");
    if !dir.is_dir() {
        return; // No observations directory — nothing to do
    }

    // Discover YAML files
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) => {
            warn!("repository sync: failed to read {}: {e}", dir.display());
            return;
        }
    };

    let mut yaml_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            yaml_paths.push(path);
        }
    }
    yaml_paths.sort();

    if yaml_paths.is_empty() {
        return;
    }

    // Parse each YAML file into a RepoObservation
    let mut repo_observations: Vec<(String, crate::repo_observations::RepoObservation)> =
        Vec::new();
    for path in &yaml_paths {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        match crate::repo_observations::parse_observation(path) {
            Ok(o) => repo_observations.push((name, o)),
            Err(e) => warn!(
                "repository sync: failed to parse observation {}: {e:#}",
                path.display()
            ),
        }
    }

    if repo_observations.is_empty() {
        return;
    }

    // Fetch project ID from the API using repo_root
    let project_id = match find_project_id_for_repo(api, repo_root).await {
        Some(id) => id,
        None => {
            warn!(
                "repository sync: cannot sync observations — unable to determine project_id for repo"
            );
            return;
        }
    };

    // Fetch existing observations from the API (all statuses, high limit)
    let existing = api
        .list_observations(&project_id, None, Some(500))
        .await
        .unwrap_or_default();

    // Index existing repo-sourced observations by their metadata.repo_file
    let mut repo_sourced: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for o in &existing {
        let is_repo = o["source"].as_str() == Some("repo")
            || o["metadata"]["source"].as_str() == Some("repo");
        if is_repo && let Some(repo_file) = o["metadata"]["repo_file"].as_str() {
            repo_sourced.insert(repo_file.to_string(), o.clone());
        }
    }

    let mut created = 0u32;
    let mut updated = 0u32;
    let mut unchanged = 0u32;

    for (name, observation) in &repo_observations {
        let repo_file = format!("{name}.yaml");

        // Build tags: keep user tags and add repo marker
        let mut tags = observation.tags.clone();
        if !tags.iter().any(|t| t == "source:repo") {
            tags.push("source:repo".to_string());
        }

        // Build metadata with source=repo and repo_file markers
        let metadata = serde_json::json!({
            "source": "repo",
            "repo_file": repo_file,
        });

        if let Some(existing_o) = repo_sourced.remove(&repo_file) {
            // Check if content changed (including kind and tags)
            let existing_tags: Vec<String> = existing_o["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let content_changed = existing_o["title"].as_str().unwrap_or("") != observation.title
                || existing_o["kind"].as_str().unwrap_or("insight") != observation.kind
                || existing_o["severity"].as_str().unwrap_or("info") != observation.severity
                || existing_o["description"].as_str().unwrap_or("") != observation.description
                || existing_tags != tags;

            if content_changed {
                let observation_id = existing_o["id"].as_str().unwrap_or("");
                let body = serde_json::json!({
                    "title": observation.title,
                    "kind": observation.kind,
                    "description": observation.description,
                    "severity": observation.severity,
                    "tags": tags,
                    "metadata": metadata,
                });
                match api.update_observation(observation_id, &body).await {
                    Ok(_) => {
                        info!(
                            "repository sync: updated repo observation '{name}' (id={observation_id})"
                        );
                        updated += 1;
                    }
                    Err(e) => {
                        warn!("repository sync: failed to update repo observation '{name}': {e}");
                    }
                }
            } else {
                unchanged += 1;
            }
        } else {
            // Create new observation
            let body = serde_json::json!({
                "title": observation.title,
                "kind": observation.kind,
                "severity": observation.severity,
                "description": observation.description,
                "tags": tags,
                "source": "repo",
                "metadata": metadata,
            });
            match api.post_observation(&project_id, &body).await {
                Ok(_) => {
                    info!("repository sync: created repo observation '{name}'");
                    created += 1;
                }
                Err(e) => warn!("repository sync: failed to create repo observation '{name}': {e}"),
            }
        }
    }

    // Warn about orphaned repo-sourced observations
    for (repo_file, o) in &repo_sourced {
        let title = o["title"].as_str().unwrap_or("unknown");
        warn!(
            "repository sync: API observation '{title}' (repo_file={repo_file}) \
             has no matching file in repo — consider removing it"
        );
    }

    let total = created + updated + unchanged;
    if total > 0 {
        info!(
            "repository sync: synced {total} repo observation(s) ({created} created, {updated} updated, {unchanged} unchanged)"
        );
    }
}

/// Find the project ID for a given repo root by listing projects and matching git_root.
async fn find_project_id_for_repo(
    api: &dyn TaskSource,
    repo_root: &std::path::Path,
) -> Option<String> {
    let projects = api.list_projects().await.ok()?;
    let repo_root_str = repo_root.display().to_string();
    for project in &projects {
        if let Some(git_root) = project["git_root"].as_str() {
            // Match by suffix — the repo_root is an absolute path, git_root is typically relative
            if repo_root_str.ends_with(git_root) || git_root == repo_root_str {
                return project["id"].as_str().map(|s| s.to_string());
            }
        }
        if let Some(repo_path) = project["repo_path"].as_str()
            && (repo_root_str.ends_with(repo_path) || repo_path == repo_root_str)
        {
            return project["id"].as_str().map(|s| s.to_string());
        }
    }
    None
}
