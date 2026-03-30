//! Local context assembly for orchestra.
//!
//! Fetches knowledge, decisions, observations, and other context data from the
//! API and assembles a context blob for workers. Caches responses with a TTL
//! to reduce API calls when multiple tasks run against the same project.
//!
//! When embedding-based semantic ranking is not available locally, falls back
//! to the API's context endpoint (which does have embeddings).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::{Value, json};

use crate::project::api::ProjectsApi;

const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// Cached entry with expiry.
struct CacheEntry {
    value: Value,
    fetched_at: Instant,
}

/// Local context assembler with per-project caching.
pub struct ContextAssembler {
    api: ProjectsApi,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl ContextAssembler {
    pub fn new(api: ProjectsApi) -> Self {
        Self {
            api,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Assemble context for a task. Fetches from cache or API.
    ///
    /// For now, this delegates to the API's context endpoint when a task_id is
    /// provided (to get semantic ranking). For project-level context without
    /// task-specific ranking, it assembles from cached individual lists.
    pub async fn assemble(&self, project_id: &str, task_id: Option<&str>) -> Result<Value> {
        // When task_id is provided, use the API's context endpoint for semantic ranking.
        // This is the fallback until we have local embedding support.
        if let Some(tid) = task_id {
            return self.api.get_context_for_task(project_id, tid).await;
        }

        // Project-level context: assemble from cached lists
        let knowledge = self
            .cached_fetch(&format!("knowledge:{project_id}"), || {
                self.api.list_knowledge(project_id, None, Some(50))
            })
            .await?;

        let decisions = self
            .cached_fetch(&format!("decisions:{project_id}"), || {
                self.api.list_decisions(project_id)
            })
            .await?;

        let observations = self
            .cached_fetch(&format!("observations:{project_id}"), || {
                self.api
                    .list_observations(project_id, Some("open"), Some(20))
            })
            .await?;

        Ok(json!({
            "knowledge": knowledge,
            "decisions": decisions,
            "observations": observations,
            "tasks": [],
        }))
    }

    /// Invalidate cache for a project (e.g., after a task modifies knowledge).
    pub fn invalidate(&self, project_id: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.retain(|k, _| !k.contains(project_id));
    }

    /// Fetch from cache if fresh, otherwise call the API and cache the result.
    async fn cached_fetch<F, Fut>(&self, key: &str, fetch: F) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Value>>>,
    {
        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(entry) = cache.get(key)
                && entry.fetched_at.elapsed() < CACHE_TTL
            {
                return Ok(entry.value.clone());
            }
        }

        // Fetch from API
        let items = fetch().await?;
        let value = json!(items);

        // Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(
                key.to_string(),
                CacheEntry {
                    value: value.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(value)
    }
}
