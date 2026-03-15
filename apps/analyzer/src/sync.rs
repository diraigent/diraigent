//! Knowledge sync — upserts analyzer outputs to the Diraigent knowledge store.
//!
//! Reads scan manifests, AI summaries, and API surface maps, then groups them
//! into per-module knowledge entries plus a dependency graph and API surface
//! entry. Uses content hashing to skip unchanged entries, so re-runs on an
//! unchanged codebase produce zero API calls.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub manifest_path: PathBuf,
    pub summaries_path: Option<PathBuf>,
    pub api_surface_path: PathBuf,
    pub cache_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    pub project_id: String,
    pub agent_id: Option<String>,
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Input types (match JSON output from scan / summarize / api-surface)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ScanManifest {
    files: Vec<ScanFile>,
}

#[derive(Debug, Deserialize)]
struct ScanFile {
    path: String,
    language: String,
    imports: Vec<String>,
    exports: Vec<ScanSymbol>,
    #[serde(default)]
    #[allow(dead_code)]
    docstring: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScanSymbol {
    name: String,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct SummaryManifestInput {
    summaries: Vec<FileSummaryInput>,
}

#[derive(Debug, Deserialize)]
struct FileSummaryInput {
    path: String,
    summary: String,
}

#[derive(Debug, Deserialize)]
struct ApiSurfaceInput {
    routes: Vec<RouteInput>,
    #[serde(default)]
    ws_messages: Vec<WsMsgInput>,
    #[serde(default)]
    traits: Vec<TraitInput>,
    #[serde(default)]
    interfaces: Vec<IfaceInput>,
}

#[derive(Debug, Deserialize)]
struct RouteInput {
    method: String,
    path: String,
    handler: String,
    file: String,
}

#[derive(Debug, Deserialize)]
struct WsMsgInput {
    type_tag: String,
    direction: String,
    fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TraitInput {
    name: String,
    file: String,
    methods: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IfaceInput {
    name: String,
    file: String,
    fields: Vec<IfaceFieldInput>,
}

#[derive(Debug, Deserialize)]
struct IfaceFieldInput {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
}

// ---------------------------------------------------------------------------
// Local sync cache (avoids API calls when nothing changed)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Default)]
struct SyncCache {
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    content_hash: String,
    knowledge_id: String,
}

impl SyncCache {
    fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self {
                version: 1,
                ..Default::default()
            };
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or(Self {
                version: 1,
                ..Default::default()
            })
    }

    fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)
    }
}

// ---------------------------------------------------------------------------
// Diraigent API types & client
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct PaginatedResponse<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct KnowledgeResp {
    id: String,
    title: String,
    #[serde(default)]
    #[allow(dead_code)]
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct KnowledgeCreate {
    title: String,
    category: String,
    content: String,
    tags: Vec<String>,
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct KnowledgeUpdate {
    content: String,
    metadata: serde_json::Value,
}

struct ApiClient {
    http: reqwest::Client,
    base: String,
    token: String,
    project_id: String,
    agent_id: Option<String>,
}

impl ApiClient {
    fn new(config: &SyncConfig) -> Self {
        // Normalise the base URL: strip trailing slash and /v1 suffix so we
        // always construct URLs as {base}/v1/...
        let mut base = config.api_url.trim_end_matches('/').to_string();
        if base.ends_with("/v1") {
            base.truncate(base.len() - 3);
        }
        Self {
            http: reqwest::Client::new(),
            base,
            token: config.api_token.clone(),
            project_id: config.project_id.clone(),
            agent_id: config.agent_id.clone(),
        }
    }

    fn req(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut r = self
            .http
            .request(method, url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json");
        if let Some(ref aid) = self.agent_id {
            r = r.header("X-Agent-Id", aid);
        }
        r
    }

    async fn list_codegen(&self) -> Result<Vec<KnowledgeResp>, String> {
        let url = format!(
            "{}/v1/{}/knowledge?tag=source:codegen&limit=100",
            self.base, self.project_id
        );
        let resp = self
            .req(reqwest::Method::GET, &url)
            .send()
            .await
            .map_err(|e| format!("HTTP error listing knowledge: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("List knowledge failed ({status}): {body}"));
        }
        let page: PaginatedResponse<KnowledgeResp> =
            resp.json().await.map_err(|e| format!("JSON parse: {e}"))?;
        Ok(page.data)
    }

    async fn create(&self, entry: &KnowledgeCreate) -> Result<KnowledgeResp, String> {
        let url = format!("{}/v1/{}/knowledge", self.base, self.project_id);
        let resp = self
            .req(reqwest::Method::POST, &url)
            .json(entry)
            .send()
            .await
            .map_err(|e| format!("HTTP error creating knowledge: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Create knowledge failed ({status}): {body}"));
        }
        resp.json().await.map_err(|e| format!("JSON parse: {e}"))
    }

    async fn update(&self, id: &str, entry: &KnowledgeUpdate) -> Result<(), String> {
        let url = format!("{}/v1/knowledge/{id}", self.base);
        let resp = self
            .req(reqwest::Method::PUT, &url)
            .json(entry)
            .send()
            .await
            .map_err(|e| format!("HTTP error updating knowledge: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Update knowledge failed ({status}): {body}"));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Content generation
// ---------------------------------------------------------------------------

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// A knowledge entry pending sync.
struct PendingEntry {
    title: String,
    content: String,
    hash: String,
}

/// Determine the top-level module for a file path.
///
/// `apps/api/src/main.rs`  → `apps/api`
/// `libs/shared/src/lib.rs` → `libs/shared`
/// `migrations/0001.sql`    → `migrations`
fn module_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 2 && (parts[0] == "apps" || parts[0] == "libs") {
        format!("{}/{}", parts[0], parts[1])
    } else if !parts.is_empty() && !parts[0].is_empty() {
        parts[0].to_string()
    } else {
        "root".to_string()
    }
}

/// Generate one knowledge entry per top-level module.
fn gen_module_entries(
    scan: &ScanManifest,
    summaries: &HashMap<String, String>,
) -> Vec<PendingEntry> {
    // Group files by module (BTreeMap for deterministic order)
    let mut modules: BTreeMap<String, Vec<&ScanFile>> = BTreeMap::new();
    for f in &scan.files {
        modules.entry(module_of(&f.path)).or_default().push(f);
    }

    modules
        .into_iter()
        .map(|(name, files)| {
            let mut c = String::new();
            c.push_str(&format!("Module with {} files.\n\n", files.len()));

            // Language breakdown
            let mut langs: BTreeMap<&str, usize> = BTreeMap::new();
            for f in &files {
                *langs.entry(&f.language).or_default() += 1;
            }
            let lang_str: Vec<String> = langs.iter().map(|(l, n)| format!("{l}: {n}")).collect();
            c.push_str(&format!("Languages: {}\n\n", lang_str.join(", ")));

            // Key exports (sorted for determinism)
            let mut exports: Vec<String> = Vec::new();
            for f in &files {
                for e in &f.exports {
                    exports.push(format!("{} ({}) — {}", e.name, e.kind, f.path));
                }
            }
            exports.sort();
            if !exports.is_empty() {
                c.push_str("Key exports:\n");
                for e in exports.iter().take(80) {
                    c.push_str(&format!("- {e}\n"));
                }
                if exports.len() > 80 {
                    c.push_str(&format!("  ... and {} more\n", exports.len() - 80));
                }
                c.push('\n');
            }

            // AI-generated file summaries
            let mut has_any = false;
            for f in &files {
                if let Some(s) = summaries.get(&f.path) {
                    if !has_any {
                        c.push_str("File summaries:\n\n");
                        has_any = true;
                    }
                    c.push_str(&format!("### {}\n{s}\n\n", f.path));
                }
            }

            let hash = content_hash(&c);
            PendingEntry {
                title: format!("Module: {name}"),
                content: c,
                hash,
            }
        })
        .collect()
}

/// Generate a dependency graph entry from the scan manifest.
///
/// Groups imports by module and lists unique import paths per module,
/// giving a structural overview of how modules depend on each other
/// and on external packages.
fn gen_dep_graph(scan: &ScanManifest) -> PendingEntry {
    let mut module_imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in &scan.files {
        let src = module_of(&f.path);
        let entry = module_imports.entry(src).or_default();
        for imp in &f.imports {
            entry.insert(imp.clone());
        }
    }

    let mut c = String::new();
    c.push_str("Module-level dependency overview extracted from static import analysis.\n\n");
    for (module, imports) in &module_imports {
        c.push_str(&format!("## {module}\n"));
        if imports.is_empty() {
            c.push_str("No imports detected.\n\n");
        } else {
            let sorted: Vec<&String> = imports.iter().collect();
            for imp in sorted.iter().take(60) {
                c.push_str(&format!("- {imp}\n"));
            }
            if sorted.len() > 60 {
                c.push_str(&format!("  ... and {} more\n", sorted.len() - 60));
            }
            c.push('\n');
        }
    }

    let hash = content_hash(&c);
    PendingEntry {
        title: "Codebase Dependency Graph".to_string(),
        content: c,
        hash,
    }
}

/// Generate an API surface knowledge entry.
fn gen_api_surface(surface: &ApiSurfaceInput) -> PendingEntry {
    let mut c = String::new();

    // HTTP routes
    c.push_str(&format!("## HTTP Routes ({})\n\n", surface.routes.len()));
    for r in &surface.routes {
        c.push_str(&format!(
            "{} {} → {} ({})\n",
            r.method, r.path, r.handler, r.file
        ));
    }
    c.push('\n');

    // WebSocket messages
    if !surface.ws_messages.is_empty() {
        c.push_str(&format!(
            "## WebSocket Messages ({})\n\n",
            surface.ws_messages.len()
        ));
        for m in &surface.ws_messages {
            let fields = if m.fields.is_empty() {
                "—".to_string()
            } else {
                m.fields.join(", ")
            };
            c.push_str(&format!(
                "{} [{}] fields: {fields}\n",
                m.type_tag, m.direction
            ));
        }
        c.push('\n');
    }

    // Rust traits
    if !surface.traits.is_empty() {
        c.push_str(&format!("## Rust Traits ({})\n\n", surface.traits.len()));
        for t in &surface.traits {
            c.push_str(&format!(
                "{} ({}) — methods: {}\n",
                t.name,
                t.file,
                t.methods.join(", ")
            ));
        }
        c.push('\n');
    }

    // TypeScript interfaces
    if !surface.interfaces.is_empty() {
        c.push_str(&format!(
            "## TypeScript Interfaces ({})\n\n",
            surface.interfaces.len()
        ));
        for i in &surface.interfaces {
            let fields: Vec<String> = i
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, f.type_name))
                .collect();
            c.push_str(&format!(
                "{} ({}) — {}\n",
                i.name,
                i.file,
                fields.join(", ")
            ));
        }
        c.push('\n');
    }

    let hash = content_hash(&c);
    PendingEntry {
        title: "API Surface Map".to_string(),
        content: c,
        hash,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub async fn run(config: SyncConfig) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load inputs
    let scan_data = std::fs::read_to_string(&config.manifest_path).map_err(|e| {
        format!(
            "cannot read manifest {}: {e}",
            config.manifest_path.display()
        )
    })?;
    let scan: ScanManifest =
        serde_json::from_str(&scan_data).map_err(|e| format!("invalid manifest JSON: {e}"))?;
    info!("Loaded scan manifest: {} files", scan.files.len());

    let summaries_map: HashMap<String, String> = match &config.summaries_path {
        Some(p) if p.exists() => {
            let data = std::fs::read_to_string(p)?;
            let sm: SummaryManifestInput = serde_json::from_str(&data)?;
            info!("Loaded {} summaries", sm.summaries.len());
            sm.summaries
                .into_iter()
                .map(|s| (s.path, s.summary))
                .collect()
        }
        Some(p) => {
            warn!(
                "Summaries file not found: {}, continuing without",
                p.display()
            );
            HashMap::new()
        }
        None => {
            info!("No summaries file provided, continuing without AI summaries");
            HashMap::new()
        }
    };

    let surface_data = std::fs::read_to_string(&config.api_surface_path).map_err(|e| {
        format!(
            "cannot read API surface {}: {e}",
            config.api_surface_path.display()
        )
    })?;
    let surface: ApiSurfaceInput = serde_json::from_str(&surface_data)
        .map_err(|e| format!("invalid API surface JSON: {e}"))?;
    info!(
        "Loaded API surface: {} routes, {} WS messages, {} traits, {} interfaces",
        surface.routes.len(),
        surface.ws_messages.len(),
        surface.traits.len(),
        surface.interfaces.len()
    );

    // 2. Generate all pending entries
    let mut pending: Vec<PendingEntry> = Vec::new();
    pending.extend(gen_module_entries(&scan, &summaries_map));
    pending.push(gen_dep_graph(&scan));
    pending.push(gen_api_surface(&surface));
    info!("Generated {} knowledge entries to sync", pending.len());

    // 3. Diff against local cache
    let mut cache = SyncCache::load(&config.cache_path);

    let mut changed: Vec<&PendingEntry> = Vec::new();
    let mut unchanged = 0usize;

    for entry in &pending {
        match cache.entries.get(&entry.title) {
            Some(cached) if cached.content_hash == entry.hash => {
                unchanged += 1;
            }
            _ => {
                changed.push(entry);
            }
        }
    }

    info!(
        "{} unchanged (cache hit), {} to sync",
        unchanged,
        changed.len()
    );

    if changed.is_empty() {
        info!("Nothing to sync — all entries up to date");
        return Ok(());
    }

    if config.dry_run {
        info!("Dry run — would sync {} entries:", changed.len());
        for e in &changed {
            let short_hash = &e.hash[..e.hash.len().min(24)];
            info!("  {} (hash: {short_hash}…)", e.title);
        }
        return Ok(());
    }

    // 4. Fetch existing remote codegen entries to find IDs for updates
    let client = ApiClient::new(&config);
    let existing = client.list_codegen().await?;
    let existing_map: HashMap<String, String> =
        existing.into_iter().map(|e| (e.title, e.id)).collect();
    info!("Found {} existing codegen entries", existing_map.len());

    // 5. Create or update changed entries
    let mut created = 0usize;
    let mut updated = 0usize;

    for entry in &changed {
        let metadata = serde_json::json!({ "content_hash": entry.hash });

        if let Some(existing_id) = existing_map.get(&entry.title) {
            // Update existing entry
            let update = KnowledgeUpdate {
                content: entry.content.clone(),
                metadata: metadata.clone(),
            };
            match client.update(existing_id, &update).await {
                Ok(()) => {
                    info!("Updated: {}", entry.title);
                    cache.entries.insert(
                        entry.title.clone(),
                        CacheEntry {
                            content_hash: entry.hash.clone(),
                            knowledge_id: existing_id.clone(),
                        },
                    );
                    updated += 1;
                }
                Err(e) => warn!("Failed to update {}: {e}", entry.title),
            }
        } else {
            // Create new entry
            let create = KnowledgeCreate {
                title: entry.title.clone(),
                category: "architecture".to_string(),
                content: entry.content.clone(),
                tags: vec!["source:codegen".to_string()],
                metadata,
            };
            match client.create(&create).await {
                Ok(resp) => {
                    info!("Created: {}", entry.title);
                    cache.entries.insert(
                        entry.title.clone(),
                        CacheEntry {
                            content_hash: entry.hash.clone(),
                            knowledge_id: resp.id,
                        },
                    );
                    created += 1;
                }
                Err(e) => warn!("Failed to create {}: {e}", entry.title),
            }
        }
    }

    // 6. Save cache
    cache.save(&config.cache_path)?;
    info!("Sync complete: {created} created, {updated} updated, {unchanged} unchanged");

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    #[test]
    fn test_content_hash_different_input() {
        let h1 = content_hash("hello");
        let h2 = content_hash("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_module_of_apps() {
        assert_eq!(module_of("apps/api/src/main.rs"), "apps/api");
        assert_eq!(module_of("apps/orchestra/src/lib.rs"), "apps/orchestra");
        assert_eq!(module_of("apps/web/src/app/app.ts"), "apps/web");
    }

    #[test]
    fn test_module_of_libs() {
        assert_eq!(module_of("libs/shared/src/lib.rs"), "libs/shared");
    }

    #[test]
    fn test_module_of_top_level() {
        assert_eq!(module_of("migrations/0001.sql"), "migrations");
        assert_eq!(module_of("Cargo.toml"), "Cargo.toml");
    }

    #[test]
    fn test_module_of_empty() {
        assert_eq!(module_of(""), "root");
    }

    #[test]
    fn test_gen_module_entries_groups_by_module() {
        let scan = ScanManifest {
            files: vec![
                ScanFile {
                    path: "apps/api/src/main.rs".to_string(),
                    language: "rust".to_string(),
                    imports: vec!["std::io".to_string()],
                    exports: vec![ScanSymbol {
                        name: "main".to_string(),
                        kind: "fn".to_string(),
                    }],
                    docstring: None,
                },
                ScanFile {
                    path: "apps/api/src/lib.rs".to_string(),
                    language: "rust".to_string(),
                    imports: vec![],
                    exports: vec![ScanSymbol {
                        name: "run".to_string(),
                        kind: "fn".to_string(),
                    }],
                    docstring: None,
                },
                ScanFile {
                    path: "apps/web/src/app.ts".to_string(),
                    language: "typescript".to_string(),
                    imports: vec![],
                    exports: vec![],
                    docstring: None,
                },
            ],
        };

        let entries = gen_module_entries(&scan, &HashMap::new());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Module: apps/api");
        assert_eq!(entries[1].title, "Module: apps/web");

        // apps/api should mention 2 files
        assert!(entries[0].content.contains("2 files"));
        // apps/web should mention 1 file
        assert!(entries[1].content.contains("1 files"));
    }

    #[test]
    fn test_gen_module_entries_includes_summaries() {
        let scan = ScanManifest {
            files: vec![ScanFile {
                path: "apps/api/src/main.rs".to_string(),
                language: "rust".to_string(),
                imports: vec![],
                exports: vec![],
                docstring: None,
            }],
        };
        let mut summaries = HashMap::new();
        summaries.insert(
            "apps/api/src/main.rs".to_string(),
            "Entry point for the API server.".to_string(),
        );

        let entries = gen_module_entries(&scan, &summaries);
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .content
                .contains("Entry point for the API server.")
        );
    }

    #[test]
    fn test_gen_module_entries_deterministic() {
        let scan = ScanManifest {
            files: vec![
                ScanFile {
                    path: "apps/api/src/a.rs".to_string(),
                    language: "rust".to_string(),
                    imports: vec![],
                    exports: vec![ScanSymbol {
                        name: "foo".to_string(),
                        kind: "fn".to_string(),
                    }],
                    docstring: None,
                },
                ScanFile {
                    path: "apps/api/src/b.rs".to_string(),
                    language: "rust".to_string(),
                    imports: vec![],
                    exports: vec![ScanSymbol {
                        name: "bar".to_string(),
                        kind: "fn".to_string(),
                    }],
                    docstring: None,
                },
            ],
        };

        let e1 = gen_module_entries(&scan, &HashMap::new());
        let e2 = gen_module_entries(&scan, &HashMap::new());
        assert_eq!(e1[0].hash, e2[0].hash);
    }

    #[test]
    fn test_gen_dep_graph() {
        let scan = ScanManifest {
            files: vec![
                ScanFile {
                    path: "apps/api/src/main.rs".to_string(),
                    language: "rust".to_string(),
                    imports: vec!["crate::routes".to_string(), "crate::models".to_string()],
                    exports: vec![],
                    docstring: None,
                },
                ScanFile {
                    path: "apps/orchestra/src/main.rs".to_string(),
                    language: "rust".to_string(),
                    imports: vec!["reqwest".to_string()],
                    exports: vec![],
                    docstring: None,
                },
            ],
        };

        let entry = gen_dep_graph(&scan);
        assert_eq!(entry.title, "Codebase Dependency Graph");
        assert!(entry.content.contains("apps/api"));
        assert!(entry.content.contains("apps/orchestra"));
        assert!(entry.content.contains("crate::routes"));
        assert!(entry.content.contains("reqwest"));
    }

    #[test]
    fn test_gen_dep_graph_deterministic() {
        let scan = ScanManifest {
            files: vec![ScanFile {
                path: "apps/api/src/main.rs".to_string(),
                language: "rust".to_string(),
                imports: vec!["b".to_string(), "a".to_string()],
                exports: vec![],
                docstring: None,
            }],
        };

        let e1 = gen_dep_graph(&scan);
        let e2 = gen_dep_graph(&scan);
        assert_eq!(e1.hash, e2.hash);
    }

    #[test]
    fn test_gen_api_surface() {
        let surface = ApiSurfaceInput {
            routes: vec![RouteInput {
                method: "GET".to_string(),
                path: "/v1/tasks".to_string(),
                handler: "list_tasks".to_string(),
                file: "routes/tasks.rs".to_string(),
            }],
            ws_messages: vec![WsMsgInput {
                type_tag: "task.updated".to_string(),
                direction: "server→client".to_string(),
                fields: vec!["task_id".to_string()],
            }],
            traits: vec![TraitInput {
                name: "DiraigentDb".to_string(),
                file: "db/mod.rs".to_string(),
                methods: vec!["get_task".to_string(), "list_tasks".to_string()],
            }],
            interfaces: vec![],
        };

        let entry = gen_api_surface(&surface);
        assert_eq!(entry.title, "API Surface Map");
        assert!(entry.content.contains("GET /v1/tasks"));
        assert!(entry.content.contains("task.updated"));
        assert!(entry.content.contains("DiraigentDb"));
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = std::env::temp_dir().join("analyzer_sync_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sync-cache.json");

        let mut cache = SyncCache {
            version: 1,
            entries: HashMap::new(),
        };
        cache.entries.insert(
            "Module: apps/api".to_string(),
            CacheEntry {
                content_hash: "sha256:abc".to_string(),
                knowledge_id: "uuid-123".to_string(),
            },
        );

        cache.save(&path).unwrap();
        let loaded = SyncCache::load(&path);

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.entries.len(), 1);
        let entry = loaded.entries.get("Module: apps/api").unwrap();
        assert_eq!(entry.content_hash, "sha256:abc");
        assert_eq!(entry.knowledge_id, "uuid-123");

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_cache_load_nonexistent() {
        let cache = SyncCache::load(Path::new("/nonexistent/sync-cache.json"));
        assert_eq!(cache.version, 1);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_load_corrupt() {
        let dir = std::env::temp_dir().join("analyzer_sync_corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corrupt.json");

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"not valid json {{{").unwrap();

        let cache = SyncCache::load(&path);
        assert_eq!(cache.version, 1);
        assert!(cache.entries.is_empty());

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_changed_entry_detected() {
        let mut cache = SyncCache {
            version: 1,
            entries: HashMap::new(),
        };
        cache.entries.insert(
            "Module: apps/api".to_string(),
            CacheEntry {
                content_hash: "sha256:old".to_string(),
                knowledge_id: "id-1".to_string(),
            },
        );

        let pending = vec![
            PendingEntry {
                title: "Module: apps/api".to_string(),
                content: "new content".to_string(),
                hash: "sha256:new".to_string(),
            },
            PendingEntry {
                title: "Module: apps/web".to_string(),
                content: "web content".to_string(),
                hash: "sha256:web".to_string(),
            },
        ];

        let mut changed = Vec::new();
        let mut unchanged = 0usize;
        for entry in &pending {
            match cache.entries.get(&entry.title) {
                Some(cached) if cached.content_hash == entry.hash => unchanged += 1,
                _ => changed.push(&entry.title),
            }
        }

        assert_eq!(unchanged, 0);
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&&"Module: apps/api".to_string()));
        assert!(changed.contains(&&"Module: apps/web".to_string()));
    }

    #[test]
    fn test_unchanged_entry_skipped() {
        let mut cache = SyncCache {
            version: 1,
            entries: HashMap::new(),
        };
        cache.entries.insert(
            "Module: apps/api".to_string(),
            CacheEntry {
                content_hash: "sha256:same".to_string(),
                knowledge_id: "id-1".to_string(),
            },
        );

        let pending = PendingEntry {
            title: "Module: apps/api".to_string(),
            content: "content".to_string(),
            hash: "sha256:same".to_string(),
        };

        let cached = cache.entries.get(&pending.title).unwrap();
        assert_eq!(cached.content_hash, pending.hash);
    }
}
