use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

use super::projects::get_project_by_id;
use super::{Table, delete_by_id, fetch_by_id};

const KNOWLEDGE_FILTERS_WHERE: &str = "WHERE project_id = $1 \
    AND ($2::text IS NULL OR category = $2) \
    AND ($3::text IS NULL OR $3 = ANY(tags))";

// ── Knowledge ──

pub async fn create_knowledge(
    pool: &PgPool,
    project_id: Uuid,
    req: &CreateKnowledge,
    created_by: Uuid,
) -> Result<Knowledge, AppError> {
    let _ = get_project_by_id(pool, project_id).await?;
    let category = req.category.as_deref().unwrap_or("general");
    let tags = req.tags.clone().unwrap_or_default();
    let metadata = req.metadata.clone().unwrap_or(serde_json::json!({}));

    let k = sqlx::query_as::<_, Knowledge>(
        "INSERT INTO diraigent.knowledge (project_id, title, category, content, tags, metadata, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
    )
    .bind(project_id)
    .bind(&req.title)
    .bind(category)
    .bind(&req.content)
    .bind(&tags)
    .bind(&metadata)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(k)
}

pub async fn get_knowledge_by_id(pool: &PgPool, id: Uuid) -> Result<Knowledge, AppError> {
    fetch_by_id(pool, Table::Knowledge, id, "Knowledge entry not found").await
}

pub async fn list_knowledge(
    pool: &PgPool,
    project_id: Uuid,
    filters: &KnowledgeFilters,
) -> Result<Vec<Knowledge>, AppError> {
    let limit = filters.limit.unwrap_or(50).min(100);
    let offset = filters.offset.unwrap_or(0);

    let sql = format!(
        "SELECT * FROM diraigent.knowledge {} ORDER BY created_at DESC LIMIT $4 OFFSET $5",
        KNOWLEDGE_FILTERS_WHERE
    );
    let items = sqlx::query_as::<_, Knowledge>(&sql)
        .bind(project_id)
        .bind(&filters.category)
        .bind(&filters.tag)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    Ok(items)
}

pub async fn update_knowledge(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateKnowledge,
) -> Result<Knowledge, AppError> {
    let existing = get_knowledge_by_id(pool, id).await?;

    let title = req.title.as_deref().unwrap_or(&existing.title);
    let category = req.category.as_deref().unwrap_or(&existing.category);
    let content = req.content.as_deref().unwrap_or(&existing.content);
    let tags = req.tags.as_ref().unwrap_or(&existing.tags);
    let metadata = req.metadata.as_ref().unwrap_or(&existing.metadata);

    let k = sqlx::query_as::<_, Knowledge>(
        "UPDATE diraigent.knowledge
         SET title = $2, category = $3, content = $4, tags = $5, metadata = $6
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(title)
    .bind(category)
    .bind(content)
    .bind(tags)
    .bind(metadata)
    .fetch_one(pool)
    .await?;

    Ok(k)
}

pub async fn delete_knowledge(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    delete_by_id(pool, Table::Knowledge, id, "Knowledge entry not found").await
}

pub async fn update_knowledge_embedding(
    pool: &PgPool,
    id: Uuid,
    embedding: &[f64],
) -> Result<(), AppError> {
    let embedding_vec: Vec<f64> = embedding.to_vec();
    sqlx::query("UPDATE diraigent.knowledge SET embedding = $2 WHERE id = $1")
        .bind(id)
        .bind(&embedding_vec)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_knowledge_with_embeddings(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Knowledge>, AppError> {
    let items = sqlx::query_as::<_, Knowledge>(
        "SELECT * FROM diraigent.knowledge WHERE project_id = $1 AND embedding IS NOT NULL ORDER BY title",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(items)
}

pub async fn count_knowledge(
    pool: &PgPool,
    project_id: Uuid,
    filters: &KnowledgeFilters,
) -> Result<i64, AppError> {
    let sql = format!(
        "SELECT COUNT(*) FROM diraigent.knowledge {}",
        KNOWLEDGE_FILTERS_WHERE
    );
    let row: (i64,) = sqlx::query_as(&sql)
        .bind(project_id)
        .bind(&filters.category)
        .bind(&filters.tag)
        .fetch_one(pool)
        .await?;

    Ok(row.0)
}
