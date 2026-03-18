use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

/// Truncate a string to at most `max_chars` characters, appending "…" if truncated.
/// Unlike byte-index slicing (`s[..n]`), this is UTF-8 safe.
pub fn truncate_snippet(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => format!("{}…", &s[..byte_idx]),
        None => s.to_string(),
    }
}

/// Common English stop words to exclude from keyword extraction.
const STOP_WORDS: &[&str] = &[
    "the", "and", "or", "is", "it", "in", "to", "for", "of", "a", "an", "on", "at", "by", "with",
    "from", "as", "be", "was", "are", "has", "have", "had", "this", "that", "will", "can",
    "should", "would", "not", "but", "if", "do", "no", "so", "up", "out",
];

/// Extract significant keywords from text fragments.
/// Splits on whitespace and punctuation, lowercases, filters out words < 3 chars
/// and common stop words.
fn extract_keywords(texts: &[&str]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut keywords = Vec::new();

    for text in texts {
        // Split on whitespace and common punctuation
        for word in text
            .split(|c: char| c.is_whitespace() || ".,;:!?()[]{}\"'`/\\|<>@#$%^&*+=~".contains(c))
        {
            let w = word.to_lowercase();
            if w.len() < 3 {
                continue;
            }
            if STOP_WORDS.contains(&w.as_str()) {
                continue;
            }
            if seen.insert(w.clone()) {
                keywords.push(w);
            }
        }
    }

    keywords
}

/// Search knowledge entries for keyword matches and return scored RelatedItems.
async fn search_knowledge(
    pool: &PgPool,
    project_id: Uuid,
    keywords: &[String],
    file_paths: &[&str],
    exclude_ids: &[Uuid],
    limit: i64,
) -> Result<Vec<RelatedItem>, AppError> {
    if keywords.is_empty() && file_paths.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch knowledge for this project, capped to avoid unbounded memory use.
    let rows = sqlx::query_as::<_, Knowledge>(
        "SELECT * FROM diraigent.knowledge WHERE project_id = $1 ORDER BY updated_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut scored: Vec<RelatedItem> = Vec::new();

    for row in &rows {
        if exclude_ids.contains(&row.id) {
            continue;
        }

        let mut score = 0.0_f64;
        let mut reasons = Vec::new();
        let title_lower = row.title.to_lowercase();
        let content_lower = row.content.to_lowercase();

        // Keyword scoring
        for kw in keywords {
            if title_lower.contains(kw.as_str()) {
                score += 1.0;
                if reasons.is_empty() || !reasons.contains(&format!("title match: {kw}")) {
                    reasons.push(format!("title match: {kw}"));
                }
            }
            if content_lower.contains(kw.as_str()) {
                score += 0.5;
                if reasons.len() < 3 {
                    reasons.push(format!("content match: {kw}"));
                }
            }
        }

        // File path boost
        for fp in file_paths {
            let fp_lower = fp.to_lowercase();
            if content_lower.contains(&fp_lower) {
                score += 0.3;
                reasons.push(format!("file path match: {fp}"));
                break; // Only boost once for file paths
            }
        }

        if score > 0.0 {
            let snippet = Some(truncate_snippet(&row.content, 120));

            scored.push(RelatedItem {
                entity_type: "knowledge".to_string(),
                id: row.id,
                title: row.title.clone(),
                snippet,
                relevance_score: score,
                reason: reasons.join("; "),
            });
        }
    }

    scored.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit as usize);
    Ok(scored)
}

/// Search decisions for keyword matches and return scored RelatedItems.
async fn search_decisions(
    pool: &PgPool,
    project_id: Uuid,
    keywords: &[String],
    file_paths: &[&str],
    exclude_ids: &[Uuid],
    limit: i64,
) -> Result<Vec<RelatedItem>, AppError> {
    if keywords.is_empty() && file_paths.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, Decision>(
        "SELECT * FROM diraigent.decision WHERE project_id = $1 ORDER BY updated_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut scored: Vec<RelatedItem> = Vec::new();

    for row in &rows {
        if exclude_ids.contains(&row.id) {
            continue;
        }

        let mut score = 0.0_f64;
        let mut reasons = Vec::new();
        let title_lower = row.title.to_lowercase();
        let context_lower = row.context.to_lowercase();
        let decision_lower = row.decision.as_deref().unwrap_or("").to_lowercase();
        let rationale_lower = row.rationale.as_deref().unwrap_or("").to_lowercase();

        for kw in keywords {
            if title_lower.contains(kw.as_str()) {
                score += 1.0;
                if reasons.is_empty() || !reasons.contains(&format!("title match: {kw}")) {
                    reasons.push(format!("title match: {kw}"));
                }
            }
            if context_lower.contains(kw.as_str()) {
                score += 0.5;
            }
            if decision_lower.contains(kw.as_str()) {
                score += 0.5;
            }
            if rationale_lower.contains(kw.as_str()) {
                score += 0.5;
            }
        }

        // File path boost
        for fp in file_paths {
            let fp_lower = fp.to_lowercase();
            if context_lower.contains(&fp_lower) || decision_lower.contains(&fp_lower) {
                score += 0.3;
                reasons.push(format!("file path match: {fp}"));
                break;
            }
        }

        if score > 0.0 {
            let snippet = Some(truncate_snippet(&row.context, 120));

            scored.push(RelatedItem {
                entity_type: "decision".to_string(),
                id: row.id,
                title: row.title.clone(),
                snippet,
                relevance_score: score,
                reason: reasons.join("; "),
            });
        }
    }

    scored.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit as usize);
    Ok(scored)
}

/// Search observations for keyword matches and return scored RelatedItems.
/// Only searches observations with status IN ('open', 'acknowledged').
async fn search_observations(
    pool: &PgPool,
    project_id: Uuid,
    keywords: &[String],
    exclude_ids: &[Uuid],
    limit: i64,
) -> Result<Vec<RelatedItem>, AppError> {
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, Observation>(
        "SELECT * FROM diraigent.observation WHERE project_id = $1 AND status IN ('open', 'acknowledged') ORDER BY updated_at DESC LIMIT 500",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut scored: Vec<RelatedItem> = Vec::new();

    for row in &rows {
        if exclude_ids.contains(&row.id) {
            continue;
        }

        let mut score = 0.0_f64;
        let mut reasons = Vec::new();
        let title_lower = row.title.to_lowercase();
        let desc_lower = row.description.as_deref().unwrap_or("").to_lowercase();

        for kw in keywords {
            if title_lower.contains(kw.as_str()) {
                score += 1.0;
                if reasons.is_empty() || !reasons.contains(&format!("title match: {kw}")) {
                    reasons.push(format!("title match: {kw}"));
                }
            }
            if desc_lower.contains(kw.as_str()) {
                score += 0.5;
                if reasons.len() < 3 {
                    reasons.push(format!("description match: {kw}"));
                }
            }
        }

        if score > 0.0 {
            let snippet = row.description.as_ref().map(|d| truncate_snippet(d, 120));

            scored.push(RelatedItem {
                entity_type: "observation".to_string(),
                id: row.id,
                title: row.title.clone(),
                snippet,
                relevance_score: score,
                reason: reasons.join("; "),
            });
        }
    }

    scored.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit as usize);
    Ok(scored)
}

/// Find related items (knowledge, decisions, observations) for a set of query texts
/// and optional file paths. Returns scored results ordered by relevance.
pub async fn find_related_items(
    pool: &PgPool,
    project_id: Uuid,
    query_texts: &[&str],
    file_paths: &[&str],
    exclude_ids: Option<Vec<Uuid>>,
    limit_per_type: i64,
) -> Result<RelatedItems, AppError> {
    let keywords = extract_keywords(query_texts);
    let exclude = exclude_ids.unwrap_or_default();

    let (knowledge, decisions, observations) = tokio::try_join!(
        search_knowledge(
            pool,
            project_id,
            &keywords,
            file_paths,
            &exclude,
            limit_per_type
        ),
        search_decisions(
            pool,
            project_id,
            &keywords,
            file_paths,
            &exclude,
            limit_per_type
        ),
        search_observations(pool, project_id, &keywords, &exclude, limit_per_type),
    )?;

    Ok(RelatedItems {
        knowledge,
        decisions,
        observations,
    })
}

/// Find related items for a specific task, extracting query texts from the task's
/// title, spec, acceptance criteria, and file paths from context.files.
/// If the task has a decision_id, that decision is included with relevance 1.0.
pub async fn find_related_for_task(
    pool: &PgPool,
    project_id: Uuid,
    task: &Task,
) -> Result<RelatedItems, AppError> {
    // Extract query texts from task fields
    let mut query_texts: Vec<String> = vec![task.title.clone()];

    // Extract spec from context JSON
    if let Some(spec) = task.context.get("spec").and_then(|v| v.as_str()) {
        query_texts.push(spec.to_string());
    } else if let Some(desc) = task.context.get("description").and_then(|v| v.as_str()) {
        query_texts.push(desc.to_string());
    }

    // Extract acceptance criteria
    if let Some(criteria) = task
        .context
        .get("acceptance_criteria")
        .and_then(|v| v.as_array())
    {
        let joined: String = criteria
            .iter()
            .filter_map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if !joined.is_empty() {
            query_texts.push(joined);
        }
    }

    // Extract file paths from context
    let file_paths: Vec<String> = task
        .context
        .get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let query_refs: Vec<&str> = query_texts.iter().map(|s| s.as_str()).collect();
    let file_refs: Vec<&str> = file_paths.iter().map(|s| s.as_str()).collect();

    // Exclude the task's own decision_id from the general search (we'll add it manually)
    let exclude_ids = task.decision_id.map(|id| vec![id]);

    let mut result =
        find_related_items(pool, project_id, &query_refs, &file_refs, exclude_ids, 10).await?;

    // If the task has a decision_id, include that decision with max relevance
    if let Some(decision_id) = task.decision_id
        && let Ok(decision) = super::fetch_by_id::<Decision>(
            pool,
            super::Table::Decision,
            decision_id,
            "Decision not found",
        )
        .await
    {
        let snippet = Some(truncate_snippet(&decision.context, 120));

        // Insert at the front with highest relevance
        result.decisions.insert(
            0,
            RelatedItem {
                entity_type: "decision".to_string(),
                id: decision.id,
                title: decision.title,
                snippet,
                relevance_score: 1.0,
                reason: "originating decision".to_string(),
            },
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords_filters_short_words() {
        let keywords = extract_keywords(&["I am a test of the system"]);
        assert!(!keywords.contains(&"am".to_string()));
        assert!(!keywords.contains(&"of".to_string()));
        assert!(!keywords.contains(&"a".to_string()));
        assert!(keywords.contains(&"test".to_string()));
        assert!(keywords.contains(&"system".to_string()));
    }

    #[test]
    fn test_extract_keywords_filters_stop_words() {
        let keywords = extract_keywords(&["the and or is it should would not but"]);
        assert!(
            keywords.is_empty(),
            "all stop words should be filtered: {keywords:?}"
        );
    }

    #[test]
    fn test_extract_keywords_splits_punctuation() {
        let keywords = extract_keywords(&["apps/api/src/models.rs"]);
        assert!(keywords.contains(&"apps".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"src".to_string()));
        assert!(keywords.contains(&"models".to_string()));
    }

    #[test]
    fn test_extract_keywords_deduplicates() {
        let keywords = extract_keywords(&["test test test"]);
        assert_eq!(
            keywords.iter().filter(|k| *k == "test").count(),
            1,
            "should deduplicate"
        );
    }

    #[test]
    fn test_extract_keywords_lowercases() {
        let keywords = extract_keywords(&["HELLO World"]);
        assert!(keywords.contains(&"hello".to_string()));
        assert!(keywords.contains(&"world".to_string()));
        assert!(!keywords.contains(&"HELLO".to_string()));
    }

    #[test]
    fn test_extract_keywords_multiple_texts() {
        let keywords = extract_keywords(&["related items", "relevance matching engine"]);
        assert!(keywords.contains(&"related".to_string()));
        assert!(keywords.contains(&"items".to_string()));
        assert!(keywords.contains(&"relevance".to_string()));
        assert!(keywords.contains(&"matching".to_string()));
        assert!(keywords.contains(&"engine".to_string()));
    }

    #[test]
    fn test_extract_keywords_empty_input() {
        let keywords = extract_keywords(&[]);
        assert!(keywords.is_empty());

        let keywords = extract_keywords(&[""]);
        assert!(keywords.is_empty());
    }
}
