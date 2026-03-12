//! In-memory package cache with TTL.
//!
//! Avoids a DB round-trip per request when validating domain enum fields
//! (task.kind, knowledge.category, observation.kind, event.kind, integration.kind).
//!
//! The cache is keyed by `project_id`. Entries expire after [`CACHE_TTL`].
//! Call [`PackageCache::invalidate`] when a project's package is changed so
//! subsequent requests see the updated allowed values immediately.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use uuid::Uuid;

use crate::db::DiraigentDb;
use crate::error::AppError;
use crate::models::Package;

/// How long a cached package entry is considered fresh.
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

#[derive(Clone)]
pub struct PackageCache {
    inner: Arc<DashMap<Uuid, (Package, Instant)>>,
    db: Arc<dyn DiraigentDb>,
}

impl PackageCache {
    pub fn new(db: Arc<dyn DiraigentDb>) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            db,
        }
    }

    /// Return the package for `project_id`, using the cache when the entry is
    /// fresh enough.  Returns `None` when the project has no package assigned
    /// (e.g. before migration 023 runs); callers fall back to the hardcoded
    /// allow-lists in that case.
    pub async fn get_for_project(&self, project_id: Uuid) -> Result<Option<Package>, AppError> {
        // Fast path: cache hit within TTL
        if let Some(entry) = self.inner.get(&project_id)
            && entry.1.elapsed() < CACHE_TTL
        {
            return Ok(Some(entry.0.clone()));
        }

        // Cache miss (or stale entry): fetch from DB
        let pkg = self.db.get_package_for_project(project_id).await?;
        if let Some(ref p) = pkg {
            self.inner.insert(project_id, (p.clone(), Instant::now()));
        } else {
            // Remove stale entry if present so we don't keep serving it
            self.inner.remove(&project_id);
        }
        Ok(pkg)
    }

    /// Remove a project's entry from the cache. Call this whenever a project's
    /// `package_id` is changed so the next request re-fetches from DB.
    pub fn invalidate(&self, project_id: Uuid) {
        self.inner.remove(&project_id);
    }
}
