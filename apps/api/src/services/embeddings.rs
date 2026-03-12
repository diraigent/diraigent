//! Embedding provider abstraction for semantic knowledge retrieval.
//!
//! # Configuration
//!
//! | Env var | Default | Description |
//! |---------|---------|-------------|
//! | `EMBEDDING_URL` | `http://localhost:11434` | Base URL of the embedding service (Ollama) |
//! | `EMBEDDING_MODEL` | `nomic-embed-text` | Model name to use for embeddings |
//! | `EMBEDDING_TOP_K` | `5` | Number of relevant knowledge entries to return |
//!
//! If `EMBEDDING_URL` is not set, a no-op provider is used and semantic search
//! falls back to returning the full knowledge list.

use std::env;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

// ── Trait ────────────────────────────────────────────────────────────────────

/// Produces a vector embedding for a piece of text.
///
/// Implementations are expected to be cheap to clone (Arc-wrapped internals).
/// Returns `Ok(None)` when the provider is unavailable or the model is not
/// loaded, allowing callers to fall back to the full knowledge list.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Option<Vec<f64>>>;
}

// ── No-op provider ───────────────────────────────────────────────────────────

/// Always returns `None`. Used when no embedding service is configured.
pub struct NullEmbedder;

#[async_trait]
impl EmbeddingProvider for NullEmbedder {
    async fn embed(&self, _text: &str) -> Result<Option<Vec<f64>>> {
        Ok(None)
    }
}

// ── Ollama provider ──────────────────────────────────────────────────────────

/// Calls the Ollama `/api/embeddings` endpoint.
pub struct OllamaEmbedder {
    client: reqwest::Client,
    url: String,
    model: String,
}

impl OllamaEmbedder {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            url: format!("{}/api/embeddings", base_url.trim_end_matches('/')),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbedder {
    async fn embed(&self, text: &str) -> Result<Option<Vec<f64>>> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": text,
        });

        let resp = match self.client.post(&self.url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, url = %self.url, "Embedding request failed");
                return Ok(None);
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(
                status = %resp.status(),
                url = %self.url,
                "Embedding service returned error"
            );
            return Ok(None);
        }

        let json: serde_json::Value = resp.json().await?;
        let embedding = json["embedding"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>());

        Ok(embedding)
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Create an embedding provider from environment variables.
///
/// Returns an `OllamaEmbedder` when `EMBEDDING_URL` is set, otherwise a
/// `NullEmbedder` (disables semantic search).
pub fn create_embedder_from_env() -> Arc<dyn EmbeddingProvider> {
    let base_url = env::var("EMBEDDING_URL").unwrap_or_default();
    if base_url.is_empty() {
        tracing::info!("EMBEDDING_URL not set — semantic knowledge search disabled");
        return Arc::new(NullEmbedder);
    }

    let model = env::var("EMBEDDING_MODEL").unwrap_or_else(|_| "nomic-embed-text".into());
    tracing::info!(url = %base_url, model = %model, "Embedding provider: Ollama");
    Arc::new(OllamaEmbedder::new(&base_url, &model))
}

/// Read the configured top-k value (number of knowledge entries to return for
/// semantic search). Defaults to 5.
pub fn top_k_from_env() -> usize {
    env::var("EMBEDDING_TOP_K")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// Compute cosine similarity between two equal-length vectors.
/// Returns 0.0 if either vector is zero-length or they differ in dimension.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
