//! AI-powered module summarizer.
//!
//! Reads a static-analysis manifest JSON, calls the Claude API for each file
//! that has more than 10 lines of code, and produces a structured summary.
//! Unchanged files (identified by SHA-256 content hash) are served from a
//! local cache file, making re-runs near-instantaneous.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Per-file summary produced by the AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub language: String,
    pub content_hash: String,
    pub summary: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Full output of a summariser run.
#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryManifest {
    pub stats: SummaryStats,
    pub summaries: Vec<FileSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryStats {
    pub total_files: usize,
    pub summarised: usize,
    pub cached: usize,
    pub skipped_small: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub estimated_cost_usd: f64,
    pub elapsed_ms: u128,
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Default)]
struct Cache {
    version: u32,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    content_hash: String,
    summary: String,
    input_tokens: u64,
    output_tokens: u64,
}

impl Cache {
    fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self {
                version: 1,
                ..Default::default()
            };
        }
        match std::fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                warn!("corrupt cache file, starting fresh: {e}");
                Self {
                    version: 1,
                    ..Default::default()
                }
            }),
            Err(e) => {
                warn!("cannot read cache file: {e}");
                Self {
                    version: 1,
                    ..Default::default()
                }
            }
        }
    }

    fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)
    }

    fn lookup(&self, file_path: &str, content_hash: &str) -> Option<&CacheEntry> {
        self.entries
            .get(file_path)
            .filter(|e| e.content_hash == content_hash)
    }

    fn insert(&mut self, file_path: String, entry: CacheEntry) {
        self.entries.insert(file_path, entry);
    }
}

// ---------------------------------------------------------------------------
// Manifest input types (from the static analyser)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AnalysisManifest {
    files: Vec<ManifestFile>,
}

#[derive(Debug, Deserialize)]
struct ManifestFile {
    path: String,
    language: String,
    imports: Vec<String>,
    exports: Vec<ManifestSymbol>,
    #[serde(default)]
    docstring: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ManifestSymbol {
    name: String,
    kind: String,
}

// ---------------------------------------------------------------------------
// Claude API types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    usage: ClaudeUsage,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    text: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct ClaudeError {
    error: ClaudeErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ClaudeErrorDetail {
    message: String,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SummarizeConfig {
    pub manifest_path: PathBuf,
    pub cache_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub root_dir: PathBuf,
    pub api_key: String,
    pub model: String,
    pub budget_usd: f64,
    pub concurrency: usize,
    pub min_loc: usize,
    pub pretty: bool,
}

// ---------------------------------------------------------------------------
// Cost estimation (Claude pricing per 1M tokens)
// ---------------------------------------------------------------------------

fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_rate, output_rate) = match model {
        m if m.contains("opus") => (15.0, 75.0),
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("haiku") => (0.25, 1.25),
        _ => (3.0, 15.0), // default to Sonnet pricing
    };
    (input_tokens as f64 * input_rate + output_tokens as f64 * output_rate) / 1_000_000.0
}

// ---------------------------------------------------------------------------
// Core logic
// ---------------------------------------------------------------------------

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn count_loc(content: &str) -> usize {
    content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//") && !t.starts_with('#') && !t.starts_with("--")
        })
        .count()
}

fn build_prompt(file: &ManifestFile, source: &str) -> String {
    let mut parts = Vec::new();
    parts.push(format!("File: {}", file.path));
    parts.push(format!("Language: {}", file.language));

    if !file.exports.is_empty() {
        let symbols: Vec<String> = file
            .exports
            .iter()
            .map(|s| format!("  {} ({})", s.name, s.kind))
            .collect();
        parts.push(format!("Exported symbols:\n{}", symbols.join("\n")));
    }

    if !file.imports.is_empty() {
        let imports: Vec<String> = file.imports.iter().map(|i| format!("  {i}")).collect();
        parts.push(format!("Imports:\n{}", imports.join("\n")));
    }

    if let Some(ref doc) = file.docstring {
        parts.push(format!("Module docstring:\n{doc}"));
    }

    parts.push(format!("Source code:\n```\n{source}\n```"));

    parts.join("\n\n")
}

const SYSTEM_PROMPT: &str = "\
You are a technical documentation assistant. Given a source code file with its \
metadata (language, imports, exports, docstring), produce a concise summary \
covering:

1. **Purpose**: What this module/file does (1-2 sentences).
2. **Key abstractions**: Main types, traits, classes, or interfaces.
3. **Notable patterns**: Design patterns, architectural decisions, or idioms used.
4. **Non-obvious dependencies**: External crates/packages or internal modules that \
   are important but not immediately obvious from the file name.

Keep the summary between 3-8 sentences. Be precise and technical. \
Do NOT repeat the file path or language. \
Do NOT use markdown headers — just plain text paragraphs.";

async fn call_claude(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    file: &ManifestFile,
    source: &str,
) -> Result<(String, u64, u64), String> {
    let prompt = build_prompt(file, source);

    let request = ClaudeRequest {
        model: model.to_string(),
        max_tokens: 512,
        system: SYSTEM_PROMPT.to_string(),
        messages: vec![ClaudeMessage {
            role: "user".to_string(),
            content: prompt,
        }],
    };

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("body read: {e}"))?;

    if !status.is_success() {
        if let Ok(err) = serde_json::from_str::<ClaudeError>(&body) {
            return Err(format!("Claude API {status}: {}", err.error.message));
        }
        return Err(format!("Claude API {status}: {body}"));
    }

    let response: ClaudeResponse =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse: {e}"))?;

    let text = response
        .content
        .into_iter()
        .map(|c| c.text)
        .collect::<Vec<_>>()
        .join("");

    Ok((
        text,
        response.usage.input_tokens,
        response.usage.output_tokens,
    ))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub async fn run(config: SummarizeConfig) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // Load manifest
    let manifest_data = std::fs::read_to_string(&config.manifest_path).map_err(|e| {
        format!(
            "cannot read manifest {}: {e}",
            config.manifest_path.display()
        )
    })?;
    let manifest: AnalysisManifest =
        serde_json::from_str(&manifest_data).map_err(|e| format!("invalid manifest JSON: {e}"))?;

    info!("Loaded manifest with {} files", manifest.files.len());

    // Load cache
    let mut cache = Cache::load(&config.cache_path);

    // Classify files
    let mut summaries: Vec<FileSummary> = Vec::new();
    let mut to_summarise: Vec<(ManifestFile, String, String)> = Vec::new(); // (file, source, hash)
    let mut skipped_small = 0usize;
    let mut cached = 0usize;

    for file in manifest.files {
        // Read source from disk (relative to root_dir)
        let source_path = config.root_dir.join(&file.path);
        let source = match std::fs::read_to_string(&source_path) {
            Ok(s) => s,
            Err(e) => {
                debug!("skipping {} (cannot read: {e})", file.path);
                continue;
            }
        };

        let loc = count_loc(&source);
        if loc <= config.min_loc {
            debug!("skipping {} ({loc} LOC <= {})", file.path, config.min_loc);
            skipped_small += 1;
            continue;
        }

        let hash = content_hash(&source);

        // Check cache
        if let Some(entry) = cache.lookup(&file.path, &hash) {
            debug!("cache hit: {}", file.path);
            summaries.push(FileSummary {
                path: file.path.clone(),
                language: file.language.clone(),
                content_hash: hash,
                summary: entry.summary.clone(),
                input_tokens: entry.input_tokens,
                output_tokens: entry.output_tokens,
            });
            cached += 1;
            continue;
        }

        to_summarise.push((file, source, hash));
    }

    info!(
        "{} files to summarise, {} cached, {} skipped (small)",
        to_summarise.len(),
        cached,
        skipped_small
    );

    // Process uncached files
    let client = reqwest::Client::new();
    let total_input = Arc::new(AtomicU64::new(0));
    let total_output = Arc::new(AtomicU64::new(0));
    let budget_remaining_cents = Arc::new(AtomicU64::new((config.budget_usd * 100.0) as u64));

    // Process with concurrency control using semaphore
    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.concurrency));
    let mut handles = Vec::new();

    for (file, source, hash) in to_summarise {
        let client = client.clone();
        let api_key = config.api_key.clone();
        let model = config.model.clone();
        let total_input = total_input.clone();
        let total_output = total_output.clone();
        let budget_remaining = budget_remaining_cents.clone();
        let sem = semaphore.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            // Check budget before calling
            let remaining_cents = budget_remaining.load(Ordering::Relaxed);
            if remaining_cents == 0 {
                warn!("budget exhausted, skipping {}", file.path);
                return None;
            }

            info!("summarising: {}", file.path);

            match call_claude(&client, &api_key, &model, &file, &source).await {
                Ok((summary, input_tok, output_tok)) => {
                    total_input.fetch_add(input_tok, Ordering::Relaxed);
                    total_output.fetch_add(output_tok, Ordering::Relaxed);

                    // Update budget
                    let cost_cents = (estimate_cost(&model, input_tok, output_tok) * 100.0) as u64;
                    budget_remaining.fetch_sub(cost_cents.min(remaining_cents), Ordering::Relaxed);

                    info!("  done: {} (in={input_tok}, out={output_tok})", file.path);

                    Some(FileSummary {
                        path: file.path,
                        language: file.language,
                        content_hash: hash,
                        summary,
                        input_tokens: input_tok,
                        output_tokens: output_tok,
                    })
                }
                Err(e) => {
                    warn!("failed to summarise {}: {e}", file.path);
                    None
                }
            }
        });
        handles.push(handle);
    }

    let mut new_summaries = Vec::new();
    for handle in handles {
        if let Ok(Some(summary)) = handle.await {
            new_summaries.push(summary);
        }
    }

    // Update cache with new summaries
    for s in &new_summaries {
        cache.insert(
            s.path.clone(),
            CacheEntry {
                content_hash: s.content_hash.clone(),
                summary: s.summary.clone(),
                input_tokens: s.input_tokens,
                output_tokens: s.output_tokens,
            },
        );
    }

    let summarised = new_summaries.len();
    summaries.extend(new_summaries);

    // Sort for deterministic output
    summaries.sort_by(|a, b| a.path.cmp(&b.path));

    // Save cache
    cache.save(&config.cache_path)?;
    info!("cache saved to {}", config.cache_path.display());

    // Compute stats
    let ti = total_input.load(Ordering::Relaxed);
    let to = total_output.load(Ordering::Relaxed);
    let cost = estimate_cost(&config.model, ti, to);

    let manifest = SummaryManifest {
        stats: SummaryStats {
            total_files: summaries.len() + skipped_small,
            summarised,
            cached,
            skipped_small,
            total_input_tokens: ti,
            total_output_tokens: to,
            estimated_cost_usd: cost,
            elapsed_ms: start.elapsed().as_millis(),
        },
        summaries,
    };

    info!(
        "Done: {} summarised, {} cached, {} skipped | tokens: {}in/{}out | cost: ${:.4} | {:.1}s",
        manifest.stats.summarised,
        manifest.stats.cached,
        manifest.stats.skipped_small,
        manifest.stats.total_input_tokens,
        manifest.stats.total_output_tokens,
        manifest.stats.estimated_cost_usd,
        start.elapsed().as_secs_f64()
    );

    // Output
    let json = if config.pretty {
        serde_json::to_string_pretty(&manifest)?
    } else {
        serde_json::to_string(&manifest)?
    };

    match config.output_path {
        Some(ref out) => {
            std::fs::write(out, &json)?;
            eprintln!(
                "Wrote {} summaries to {} in {}ms",
                manifest.stats.summarised + manifest.stats.cached,
                out.display(),
                manifest.stats.elapsed_ms
            );
        }
        None => println!("{json}"),
    }

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
    fn test_count_loc_basic() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        assert_eq!(count_loc(code), 3);
    }

    #[test]
    fn test_count_loc_skips_comments_and_blanks() {
        let code = "\
// comment
# attribute

fn main() {
    // inner comment
    println!(\"hello\");
}
-- sql comment
";
        // Only fn, println!, } count as LOC
        assert_eq!(count_loc(code), 3);
    }

    #[test]
    fn test_count_loc_empty() {
        assert_eq!(count_loc(""), 0);
        assert_eq!(count_loc("\n\n\n"), 0);
        assert_eq!(count_loc("// just a comment"), 0);
    }

    #[test]
    fn test_cache_lookup_hit() {
        let mut cache = Cache {
            version: 1,
            entries: HashMap::new(),
        };
        cache.insert(
            "foo.rs".to_string(),
            CacheEntry {
                content_hash: "sha256:abc".to_string(),
                summary: "A module".to_string(),
                input_tokens: 100,
                output_tokens: 50,
            },
        );

        let result = cache.lookup("foo.rs", "sha256:abc");
        assert!(result.is_some());
        assert_eq!(result.unwrap().summary, "A module");
    }

    #[test]
    fn test_cache_lookup_miss_different_hash() {
        let mut cache = Cache {
            version: 1,
            entries: HashMap::new(),
        };
        cache.insert(
            "foo.rs".to_string(),
            CacheEntry {
                content_hash: "sha256:abc".to_string(),
                summary: "A module".to_string(),
                input_tokens: 100,
                output_tokens: 50,
            },
        );

        // Different hash → miss
        assert!(cache.lookup("foo.rs", "sha256:def").is_none());
    }

    #[test]
    fn test_cache_lookup_miss_no_entry() {
        let cache = Cache {
            version: 1,
            entries: HashMap::new(),
        };
        assert!(cache.lookup("bar.rs", "sha256:abc").is_none());
    }

    #[test]
    fn test_cache_save_load_roundtrip() {
        let dir = std::env::temp_dir().join("analyzer_test_cache");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_cache.json");

        let mut cache = Cache {
            version: 1,
            entries: HashMap::new(),
        };
        cache.insert(
            "src/main.rs".to_string(),
            CacheEntry {
                content_hash: "sha256:abc123".to_string(),
                summary: "Main entry point".to_string(),
                input_tokens: 200,
                output_tokens: 100,
            },
        );

        cache.save(&path).unwrap();
        let loaded = Cache::load(&path);

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.entries.len(), 1);
        let entry = loaded.lookup("src/main.rs", "sha256:abc123").unwrap();
        assert_eq!(entry.summary, "Main entry point");
        assert_eq!(entry.input_tokens, 200);

        // Cleanup
        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_cache_load_nonexistent() {
        let cache = Cache::load(Path::new("/nonexistent/path/cache.json"));
        assert_eq!(cache.version, 1);
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_load_corrupt() {
        let dir = std::env::temp_dir().join("analyzer_test_corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corrupt.json");

        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"not valid json {{{").unwrap();

        let cache = Cache::load(&path);
        assert_eq!(cache.version, 1);
        assert!(cache.entries.is_empty());

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn test_estimate_cost_sonnet() {
        let cost = estimate_cost("claude-sonnet-4-20250514", 1_000_000, 100_000);
        // 1M * $3/1M + 100K * $15/1M = $3.00 + $1.50 = $4.50
        assert!((cost - 4.5).abs() < 0.001);
    }

    #[test]
    fn test_estimate_cost_haiku() {
        let cost = estimate_cost("claude-haiku-3", 1_000_000, 1_000_000);
        // 1M * $0.25/1M + 1M * $1.25/1M = $0.25 + $1.25 = $1.50
        assert!((cost - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_build_prompt_includes_metadata() {
        let file = ManifestFile {
            path: "src/lib.rs".to_string(),
            language: "rust".to_string(),
            imports: vec!["std::io".to_string()],
            exports: vec![ManifestSymbol {
                name: "run".to_string(),
                kind: "fn".to_string(),
            }],
            docstring: Some("Module docs".to_string()),
        };
        let source = "pub fn run() {}";
        let prompt = build_prompt(&file, source);

        assert!(prompt.contains("File: src/lib.rs"));
        assert!(prompt.contains("Language: rust"));
        assert!(prompt.contains("std::io"));
        assert!(prompt.contains("run (fn)"));
        assert!(prompt.contains("Module docs"));
        assert!(prompt.contains("pub fn run() {}"));
    }
}
