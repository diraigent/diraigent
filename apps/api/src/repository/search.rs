use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

pub async fn search(
    pool: &PgPool,
    project_id: Uuid,
    query: &str,
    entity_types: Option<&[&str]>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<SearchResult>, i64), AppError> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));

    let search_tasks = entity_types.is_none_or(|t| t.contains(&"task"));
    let search_knowledge = entity_types.is_none_or(|t| t.contains(&"knowledge"));
    let search_decisions = entity_types.is_none_or(|t| t.contains(&"decision"));
    let search_observations = entity_types.is_none_or(|t| t.contains(&"observation"));

    // Build UNION ALL query dynamically based on requested entity types
    let mut parts: Vec<&str> = Vec::new();

    if search_tasks {
        parts.push(
            "SELECT 'task'::text AS entity_type, id, title,
                    LEFT(context::text, 200) AS snippet,
                    CASE
                        WHEN LOWER(title) = LOWER($2) THEN 1.0
                        WHEN LOWER(title) LIKE LOWER($2) || '%' THEN 0.9
                        WHEN LOWER(title) LIKE $3 THEN 0.7
                        ELSE 0.3
                    END::real AS relevance,
                    created_at
             FROM diraigent.task
             WHERE project_id = $1
               AND (title ILIKE $3 OR context::text ILIKE $3)",
        );
    }

    if search_knowledge {
        parts.push(
            "SELECT 'knowledge'::text AS entity_type, id, title,
                    LEFT(content, 200) AS snippet,
                    CASE
                        WHEN LOWER(title) = LOWER($2) THEN 1.0
                        WHEN LOWER(title) LIKE LOWER($2) || '%' THEN 0.9
                        WHEN LOWER(title) LIKE $3 THEN 0.7
                        ELSE 0.3
                    END::real AS relevance,
                    created_at
             FROM diraigent.knowledge
             WHERE project_id = $1
               AND (title ILIKE $3 OR content ILIKE $3)",
        );
    }

    if search_decisions {
        parts.push(
            "SELECT 'decision'::text AS entity_type, id, title,
                    LEFT(context, 200) AS snippet,
                    CASE
                        WHEN LOWER(title) = LOWER($2) THEN 1.0
                        WHEN LOWER(title) LIKE LOWER($2) || '%' THEN 0.9
                        WHEN LOWER(title) LIKE $3 THEN 0.7
                        ELSE 0.3
                    END::real AS relevance,
                    created_at
             FROM diraigent.decision
             WHERE project_id = $1
               AND (title ILIKE $3 OR context ILIKE $3 OR COALESCE(decision, '') ILIKE $3)",
        );
    }

    if search_observations {
        parts.push(
            "SELECT 'observation'::text AS entity_type, id, title,
                    LEFT(COALESCE(description, ''), 200) AS snippet,
                    CASE
                        WHEN LOWER(title) = LOWER($2) THEN 1.0
                        WHEN LOWER(title) LIKE LOWER($2) || '%' THEN 0.9
                        WHEN LOWER(title) LIKE $3 THEN 0.7
                        ELSE 0.3
                    END::real AS relevance,
                    created_at
             FROM diraigent.observation
             WHERE project_id = $1
               AND (title ILIKE $3 OR COALESCE(description, '') ILIKE $3)",
        );
    }

    if parts.is_empty() {
        return Ok((vec![], 0));
    }

    let union_sql = parts.join(" UNION ALL ");

    let count_sql = format!("SELECT COUNT(*) FROM ({}) AS s", union_sql);
    let total: (i64,) = sqlx::query_as(&count_sql)
        .bind(project_id)
        .bind(query)
        .bind(&pattern)
        .fetch_one(pool)
        .await?;

    let data_sql = format!(
        "SELECT * FROM ({}) AS s ORDER BY relevance DESC, created_at DESC LIMIT $4 OFFSET $5",
        union_sql
    );

    let rows: Vec<SearchResult> = sqlx::query_as(&data_sql)
        .bind(project_id)
        .bind(query)
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok((rows, total.0))
}
