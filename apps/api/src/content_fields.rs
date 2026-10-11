//! Large project fields are immutable Orchestra objects. Postgres retains only
//! their verified references and the metadata needed to coordinate work.
use crate::{AppState, auth::AuthUser, authz::OptionalAgentId, db::PostgresDb, error::AppError};
use axum::{
    extract::{FromRequestParts, Request, State},
    middleware::Next,
    response::Response,
};
use diraigent_types::project_content::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

tokio::task_local! { static CALLER: Option<(Uuid, Option<Uuid>)>; static PUBLISHED: Uuid; }

/// Database hydration can precede a handler's entity-level permission check.
/// Carry the authenticated caller so remote reads enforce membership first.
pub async fn caller_scope(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let (mut parts, body) = req.into_parts();
    let caller = match AuthUser::from_request_parts(&mut parts, &state).await {
        Ok(AuthUser(user)) => match OptionalAgentId::from_request_parts(&mut parts, &state).await {
            Ok(OptionalAgentId(agent)) => Some((user, agent)),
            Err(_) => None,
        },
        Err(_) => None,
    };
    let req = Request::from_parts(parts, body);
    if caller.is_none() {
        return next.run(req).await;
    }
    CALLER.scope(caller, next.run(req)).await
}

pub fn fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "project" => &["description"],
        "task" => &["context"],
        "task_update" | "task_comment" | "work_comment" => &["content", "metadata"],
        "work" => &["description", "success_criteria", "metadata"],
        "knowledge" => &["content", "metadata"],
        "decision" => &[
            "context",
            "decision",
            "rationale",
            "alternatives",
            "consequences",
        ],
        "observation" => &["description", "evidence", "metadata"],
        "event" => &["description", "metadata"],
        "audit_log" => &["before_state", "after_state", "metadata"],
        "report" => &["prompt", "result", "metadata"],
        "verification" => &["detail", "evidence"],
        "webhook_delivery" => &["payload", "response_body"],
        "webhook_dead_letter" => &["payload", "last_response_body"],
        _ => &[],
    }
}
fn json_field(kind: &str, field: &str) -> bool {
    matches!(
        field,
        "alternatives"
            | "metadata"
            | "evidence"
            | "success_criteria"
            | "payload"
            | "before_state"
            | "after_state"
    ) || (kind == "task" && field == "context")
}
fn encode_field(kind: &str, field: &str, reference: &FieldRef, original: &Value) -> Value {
    // The repository decodes alternatives into a typed array. Preserve that
    // storage shape while reserving one name as the reference envelope.
    if kind == "decision" && field == "alternatives" {
        return serde_json::json!([{"name":reference.encode(&Value::String(String::new()))}]);
    }
    reference.encode(original)
}
fn error(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}
pub fn checksum(value: &Value) -> Result<(String, usize), AppError> {
    let bytes = serde_json::to_vec(value).map_err(error)?;
    Ok((hex::encode(Sha256::digest(&bytes)), bytes.len()))
}
fn nonempty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Array(a) => !a.is_empty(),
        _ => true,
    }
}

#[derive(Clone)]
pub struct FieldStore {
    pool: PgPool,
    registry: Arc<crate::ws_registry::WsRegistry>,
}
impl FieldStore {
    pub fn new(pool: PgPool, registry: Arc<crate::ws_registry::WsRegistry>) -> Self {
        Self { pool, registry }
    }
    pub async fn enabled(&self, project: Uuid) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar(
            "SELECT heavy_content FROM diraigent.project_content_owner WHERE project_id=$1",
        )
        .bind(project)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(false))
    }
    async fn request(&self, project: Uuid, request: ContentRequest) -> Result<Value, AppError> {
        let agent = sqlx::query_scalar(
            "SELECT agent_id FROM diraigent.project_content_owner WHERE project_id=$1",
        )
        .bind(project)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::ServiceUnavailable("Project storage owner missing".into()))?;
        crate::project_content::request_parts(&self.pool, &self.registry, project, agent, request)
            .await
    }
    async fn authorize_read(&self, project: Uuid) -> Result<(), AppError> {
        if PUBLISHED.try_with(|id| *id == project).unwrap_or(false) {
            return Ok(());
        }
        match CALLER.try_with(|caller| *caller) {
            Ok(Some((user, agent))) => {
                crate::authz::require_membership(
                    &PostgresDb::new(self.pool.clone()),
                    agent,
                    user,
                    project,
                )
                .await
            }
            Ok(None) => Err(AppError::Unauthorized(
                "Content requires authentication".into(),
            )),
            // Background services have no HTTP caller and operate on trusted IDs.
            Err(_) => Ok(()),
        }
    }
    pub async fn put(&self, project: Uuid, value: &Value) -> Result<FieldRef, AppError> {
        let (hash, bytes) = checksum(value)?;
        if bytes > 8 * 1024 * 1024 {
            return Err(AppError::Validation("Project field exceeds 8 MiB".into()));
        }
        let reference = FieldRef {
            project,
            id: Uuid::now_v7(),
            checksum: hash,
            bytes,
        };
        self.request(
            project,
            ContentRequest::Put {
                kind: ContentKind::Field,
                id: reference.id,
                value: value.clone(),
            },
        )
        .await?;
        // Verify before committing a reference or clearing a legacy body.
        if self
            .request(
                project,
                ContentRequest::Get {
                    kind: ContentKind::Field,
                    id: reference.id,
                },
            )
            .await?
            != *value
        {
            return Err(AppError::ServiceUnavailable(
                "Content verification failed".into(),
            ));
        }
        Ok(reference)
    }
    pub async fn get(&self, reference: &FieldRef) -> Result<Value, AppError> {
        self.authorize_read(reference.project).await?;
        let v = self
            .request(
                reference.project,
                ContentRequest::Get {
                    kind: ContentKind::Field,
                    id: reference.id,
                },
            )
            .await
            .map_err(|e| match e {
                AppError::NotFound(_) => {
                    AppError::ServiceUnavailable("Referenced project content is missing".into())
                }
                other => other,
            })?;
        let (hash, bytes) = checksum(&v)?;
        if hash != reference.checksum || bytes != reference.bytes {
            return Err(AppError::ServiceUnavailable(
                "Stored project content checksum mismatch".into(),
            ));
        }
        Ok(v)
    }
    pub async fn prepare<T: Serialize + DeserializeOwned>(
        &self,
        project: Uuid,
        kind: &str,
        req: &T,
    ) -> Result<T, AppError> {
        let mut value = serde_json::to_value(req).map_err(error)?;
        for &field in fields(kind) {
            if value
                .get(field)
                .is_some_and(|v| FieldRef::parse(v).map_or(true, |r| r.is_some()))
            {
                return Err(AppError::Validation(
                    "Reserved project-content reference in request".into(),
                ));
            }
        }
        if self.enabled(project).await? {
            for &field in fields(kind) {
                if let Some(original) = value.get(field).filter(|v| nonempty(v)).cloned() {
                    // Small metadata is operational data; larger metadata is content.
                    if field == "metadata"
                        && serde_json::to_vec(&original).map_err(error)?.len() <= 1024
                    {
                        continue;
                    }
                    if field == "context"
                        && original
                            .get("worker")
                            .is_some_and(|worker| worker.to_string().len() > 4096)
                    {
                        return Err(AppError::Validation(
                            "Task worker configuration exceeds 4 KiB".into(),
                        ));
                    }
                    let reference = self.put(project, &original).await?;
                    value[field] = encode_field(kind, field, &reference, &original);
                }
            }
        }
        serde_json::from_value(value).map_err(error)
    }
    pub async fn project(&self, kind: &str, value: &Value) -> Result<Uuid, AppError> {
        if let Some(project) = value.get("project_id").and_then(Value::as_str) {
            return project.parse().map_err(error);
        }
        if kind == "project" {
            return value["id"]
                .as_str()
                .ok_or_else(|| error("Missing project identity"))?
                .parse()
                .map_err(error);
        }
        let (table, key) = match kind {
            "task_comment" | "task_update" => ("task", "task_id"),
            "work_comment" => ("work", "work_id"),
            "webhook_delivery" | "webhook_dead_letter" => ("webhook", "webhook_id"),
            _ => return Err(error("Missing project identity")),
        };
        let id: Uuid = value[key]
            .as_str()
            .ok_or_else(|| error("Missing parent identity"))?
            .parse()
            .map_err(error)?;
        self.parent_project(table, id).await
    }
    pub async fn parent_project(&self, table: &str, id: Uuid) -> Result<Uuid, AppError> {
        let query = match table {
            "task" => "SELECT project_id FROM diraigent.task WHERE id=$1",
            "work" => "SELECT project_id FROM diraigent.work WHERE id=$1",
            "webhook" => "SELECT project_id FROM diraigent.webhook WHERE id=$1",
            "knowledge" => "SELECT project_id FROM diraigent.knowledge WHERE id=$1",
            "decision" => "SELECT project_id FROM diraigent.decision WHERE id=$1",
            "observation" => "SELECT project_id FROM diraigent.observation WHERE id=$1",
            "report" => "SELECT project_id FROM diraigent.report WHERE id=$1",
            "verification" => "SELECT project_id FROM diraigent.verification WHERE id=$1",
            _ => return Err(error("Unknown content entity")),
        };
        sqlx::query_scalar(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Content entity not found".into()))
    }
    pub async fn store_embedding(&self, id: Uuid, embedding: &[f64]) -> Result<bool, AppError> {
        let project = self.parent_project("knowledge", id).await?;
        if !self.enabled(project).await? {
            return Ok(false);
        }
        let reference = self
            .put(project, &serde_json::to_value(embedding).map_err(error)?)
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM diraigent.knowledge WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO diraigent.knowledge_embedding_ref VALUES ($1,$2,$3,$4) ON CONFLICT(knowledge_id) DO UPDATE SET object_id=EXCLUDED.object_id,checksum=EXCLUDED.checksum,bytes=EXCLUDED.bytes")
            .bind(id).bind(reference.id).bind(&reference.checksum).bind(reference.bytes as i64).execute(&mut *tx).await?;
        sqlx::query("UPDATE diraigent.knowledge SET embedding=NULL WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn embedding(&self, project: Uuid, id: Uuid) -> Result<Option<Vec<f64>>, AppError> {
        let row: Option<(Uuid,String,i64)> = sqlx::query_as("SELECT object_id,checksum,bytes FROM diraigent.knowledge_embedding_ref WHERE knowledge_id=$1")
            .bind(id).fetch_optional(&self.pool).await?;
        match row {
            Some((id, checksum, bytes)) => Ok(Some(
                serde_json::from_value(
                    self.get(&FieldRef {
                        project,
                        id,
                        checksum,
                        bytes: bytes as usize,
                    })
                    .await?,
                )
                .map_err(error)?,
            )),
            None => Ok(None),
        }
    }
    pub async fn rank(
        &self,
        project: Uuid,
        categories: &[String],
        vector: Vec<f64>,
        limit: usize,
    ) -> Result<Option<Vec<Uuid>>, AppError> {
        if !self.enabled(project).await? {
            return Ok(None);
        }
        self.authorize_read(project).await?;
        // Until the legacy vectors have migrated, include both locations via
        // the compatibility path rather than ranking only the migrated subset.
        let legacy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM diraigent.knowledge WHERE project_id=$1 AND embedding IS NOT NULL)").bind(project).fetch_one(&self.pool).await?;
        if legacy {
            return Ok(None);
        }
        let rows: Vec<(Uuid,Uuid,String,i64)> = sqlx::query_as("SELECT k.id,r.object_id,r.checksum,r.bytes FROM diraigent.knowledge k JOIN diraigent.knowledge_embedding_ref r ON r.knowledge_id=k.id WHERE k.project_id=$1 AND (cardinality($2::text[])=0 OR k.category=ANY($2)) ORDER BY k.id LIMIT 10001")
            .bind(project).bind(categories).fetch_all(&self.pool).await?;
        if rows.is_empty() {
            return Ok(None);
        }
        if rows.len() > 10_000 {
            return Err(AppError::Validation(
                "Knowledge ranking exceeds 10000 indexed records".into(),
            ));
        }
        let candidates = rows
            .into_iter()
            .map(|(id, object, checksum, bytes)| {
                (
                    id,
                    FieldRef {
                        project,
                        id: object,
                        checksum,
                        bytes: bytes as usize,
                    },
                )
            })
            .collect();
        let v = self
            .request(
                project,
                ContentRequest::RankFields {
                    candidates,
                    vector,
                    limit: limit.min(100),
                },
            )
            .await?;
        Ok(Some(serde_json::from_value(v).map_err(error)?))
    }
    async fn hydrate_value(&self, kind: &str, value: &mut Value) -> Result<(), AppError> {
        // Iterative traversal keeps list/tuple responses compatible without a
        // recursive async future. Only model fields, never arbitrary JSON, hydrate.
        let mut records = match value {
            Value::Array(items) => items.iter_mut().collect::<Vec<_>>(),
            Value::Null => return Ok(()),
            other => vec![other],
        };
        while let Some(record) = records.pop() {
            if let Value::Array(items) = record {
                records.extend(items.iter_mut());
                continue;
            }
            if !record.is_object() {
                continue;
            }
            let mut project = None;
            for &field in fields(kind) {
                if let Some(v) = record.get(field)
                    && let Some(reference) = FieldRef::parse(v).map_err(error)?
                {
                    let expected = match project {
                        Some(p) => p,
                        None => {
                            let p = self.project(kind, record).await?;
                            project = Some(p);
                            p
                        }
                    };
                    if reference.project != expected {
                        return Err(AppError::Forbidden(
                            "Content reference belongs to another project".into(),
                        ));
                    }
                    record[field] = self.get(&reference).await?;
                }
            }
        }
        Ok(())
    }
    /// Publication is checked on central metadata before any local content read.
    /// This exception is limited to the explicitly published project, including
    /// when an authenticated nonmember visits its anonymous spectator URL.
    pub async fn hydrate_published<T: Serialize + DeserializeOwned>(
        &self,
        project: Uuid,
        kind: &str,
        result: T,
    ) -> Result<T, AppError> {
        let db = PostgresDb::new(self.pool.clone());
        let record = crate::db::DiraigentDb::get_project_by_id(&db, project).await?;
        let tenant = crate::db::DiraigentDb::get_tenant_by_id(&db, record.tenant_id).await?;
        crate::spectator::ensure_access(&record, Some(&tenant))?;
        PUBLISHED.scope(project, self.hydrate(kind, result)).await
    }
    pub async fn hydrate<T: Serialize + DeserializeOwned>(
        &self,
        kind: &str,
        result: T,
    ) -> Result<T, AppError> {
        let mut v = serde_json::to_value(result).map_err(error)?;
        self.hydrate_value(kind, &mut v).await?;
        serde_json::from_value(v).map_err(error)
    }
    pub async fn related(
        &self,
        task: &crate::models::Task,
        limit: usize,
    ) -> Result<crate::models::RelatedItems, AppError> {
        use crate::models::{RelatedItem, RelatedItems};
        let texts = [
            task.title.as_str(),
            task.context["spec"].as_str().unwrap_or(""),
            task.context["description"].as_str().unwrap_or(""),
        ];
        let mut keywords = crate::repository::extract_keywords(&texts);
        if let Some(criteria) = task.context["acceptance_criteria"].as_array() {
            for criterion in criteria.iter().filter_map(Value::as_str) {
                keywords.extend(crate::repository::extract_keywords(&[criterion]));
            }
        }
        if let Some(files) = task.context["files"].as_array() {
            keywords.extend(files.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        // Bound query work; each search stays on the owner and returns snippets.
        let mut seen = std::collections::HashSet::new();
        keywords.retain(|keyword| seen.insert(keyword.clone()));
        keywords.truncate(20);
        let mut items = std::collections::HashMap::<Uuid, RelatedItem>::new();
        for keyword in keywords {
            let (hits, _) = self
                .search_fields(
                    task.project_id,
                    &keyword,
                    Some(&["knowledge", "decision", "observation"]),
                    100,
                    0,
                    true,
                )
                .await?;
            for hit in hits {
                if Some(hit.id) == task.decision_id {
                    continue;
                }
                let item = items.entry(hit.id).or_insert_with(|| RelatedItem {
                    entity_type: hit.entity_type,
                    id: hit.id,
                    title: hit.title,
                    snippet: hit.snippet,
                    relevance_score: 0.0,
                    reason: String::new(),
                });
                item.relevance_score += f64::from(hit.relevance);
                if item.reason.len() < 200 {
                    if !item.reason.is_empty() {
                        item.reason.push_str("; ");
                    }
                    item.reason.push_str(&format!("keyword match: {keyword}"));
                }
            }
        }
        let mut result = RelatedItems {
            knowledge: vec![],
            decisions: vec![],
            observations: vec![],
        };
        for item in items.into_values() {
            match item.entity_type.as_str() {
                "knowledge" => result.knowledge.push(item),
                "decision" => result.decisions.push(item),
                _ => result.observations.push(item),
            }
        }
        for group in [
            &mut result.knowledge,
            &mut result.decisions,
            &mut result.observations,
        ] {
            group.sort_by(|a, b| {
                b.relevance_score
                    .total_cmp(&a.relevance_score)
                    .then_with(|| a.id.cmp(&b.id))
            });
            group.truncate(limit);
        }
        if let Some(id) = task.decision_id {
            let db = PostgresDb::new(self.pool.clone()).with_content(self.registry.clone());
            let decision = crate::db::DiraigentDb::get_decision_by_id(&db, id).await?;
            if decision.project_id == task.project_id {
                result.decisions.insert(
                    0,
                    RelatedItem {
                        entity_type: "decision".into(),
                        id,
                        title: decision.title,
                        snippet: Some(crate::repository::truncate_snippet(&decision.context, 120)),
                        relevance_score: 1.0,
                        reason: "originating decision".into(),
                    },
                );
                result.decisions.truncate(limit);
            }
        }
        Ok(result)
    }
    pub async fn search(
        &self,
        project: Uuid,
        query: &str,
        types: Option<&[&str]>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::models::SearchResult>, i64), AppError> {
        self.search_fields(project, query, types, limit, offset, false)
            .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn search_fields(
        &self,
        project: Uuid,
        query: &str,
        types: Option<&[&str]>,
        limit: i64,
        offset: i64,
        active_observations: bool,
    ) -> Result<(Vec<crate::models::SearchResult>, i64), AppError> {
        self.authorize_read(project).await?;
        let mut candidates = Vec::new();
        for kind in ["task", "knowledge", "decision", "observation"] {
            if types.is_some_and(|types| !types.contains(&kind)) {
                continue;
            }
            let selections = fields(kind).iter().map(|field| {
                let marker = if *field == "alternatives" {
                    format!("t.{field}->0->>'name' LIKE 'orchestra-field:v1:%'")
                } else if json_field(kind, field) {
                    format!("(jsonb_typeof(t.{field})='object' AND t.{field} ? '__orchestra_field_v1') OR t.{field}::text LIKE '\"orchestra-field:v1:%'")
                } else { format!("t.{field} LIKE 'orchestra-field:v1:%'") };
                // During migration SQL returns only a bounded matching snippet,
                // never a legacy document body. Afterwards it returns references.
                format!("CASE WHEN {marker} THEN to_jsonb(t.{field}) WHEN strpos(lower(t.{field}::text),lower($2))>0 THEN to_jsonb(substring(t.{field}::text FROM greatest(strpos(lower(t.{field}::text),lower($2)),1) FOR 200)) ELSE NULL END")
            }).collect::<Vec<_>>().join(",");
            let scope = if active_observations && kind == "observation" {
                " AND status IN ('open','acknowledged')"
            } else {
                ""
            };
            let sql = format!(
                "SELECT jsonb_build_object('id',t.id,'title',t.title,'created_at',t.created_at,'content_fields',jsonb_build_array({selections})) FROM diraigent.{kind} t WHERE project_id=$1{scope} ORDER BY id LIMIT 10001"
            );
            let rows: Vec<Value> = sqlx::query_scalar(&sql)
                .bind(project)
                .bind(query)
                .fetch_all(&self.pool)
                .await?;
            if candidates.len() + rows.len() > 10_000 {
                return Err(AppError::Validation(
                    "Project search exceeds 10000 indexed records".into(),
                ));
            }
            for row in rows {
                candidates.push(ContentSearchCandidate {
                    entity_type: kind.into(),
                    id: serde_json::from_value(row["id"].clone()).map_err(error)?,
                    title: row["title"].as_str().unwrap_or("").into(),
                    created_at: row["created_at"].as_str().unwrap_or("").into(),
                    fields: row["content_fields"]
                        .as_array()
                        .ok_or_else(|| error("Invalid search projection"))?
                        .iter()
                        .filter(|v| nonempty(v))
                        .cloned()
                        .collect(),
                });
            }
        }
        let v = self
            .request(
                project,
                ContentRequest::SearchFields {
                    candidates,
                    query: query.into(),
                    limit: limit.clamp(1, 100) as usize,
                    offset: offset.max(0) as usize,
                },
            )
            .await?;
        Ok((
            serde_json::from_value(v["data"].clone()).map_err(error)?,
            v["total"]
                .as_i64()
                .ok_or_else(|| error("Invalid search count"))?,
        ))
    }
}

/// Bounded, restartable legacy migration. A compare-and-set prevents replacing
/// a field concurrently edited after its snapshot was exported and verified.
pub async fn migrate(state: &AppState, project: Uuid) -> Result<usize, AppError> {
    let store = FieldStore::new(state.pool.clone(), state.ws_registry.clone());
    if !store.enabled(project).await? {
        return Ok(0);
    }
    let mut moved = 0;
    for kind in [
        "project",
        "task",
        "task_update",
        "task_comment",
        "work",
        "work_comment",
        "knowledge",
        "decision",
        "observation",
        "event",
        "audit_log",
        "report",
        "verification",
        "webhook_delivery",
        "webhook_dead_letter",
    ] {
        let scope = match kind {
            "project" => "t.id=$1".to_owned(),
            "task_update" | "task_comment" => {
                "t.task_id IN (SELECT id FROM diraigent.task WHERE project_id=$1)".into()
            }
            "work_comment" => {
                "t.work_id IN (SELECT id FROM diraigent.work WHERE project_id=$1)".into()
            }
            "webhook_delivery" | "webhook_dead_letter" => {
                "t.webhook_id IN (SELECT id FROM diraigent.webhook WHERE project_id=$1)".into()
            }
            _ => "t.project_id=$1".into(),
        };
        // Whitelisted identifiers only. Metadata smaller than 1 KiB is retained.
        let eligible = fields(kind).iter().map(|field| {
            let minimum = if *field == "metadata" { " AND octet_length(t.metadata::text)>1024" } else { "" };
            let marker = if *field == "alternatives" {
                format!("coalesce(t.{field}->0->>'name','') NOT LIKE 'orchestra-field:v1:%'")
            } else if json_field(kind, field) {
                format!("NOT (jsonb_typeof(t.{field})='object' AND t.{field} ? '__orchestra_field_v1') AND t.{field}::text NOT LIKE '\"orchestra-field:v1:%'")
            } else { format!("t.{field} NOT LIKE 'orchestra-field:v1:%'") };
            format!("(t.{field} IS NOT NULL AND t.{field}::text NOT IN ('','\"\"','{{}}','[]','null') AND {marker}{minimum})")
        }).collect::<Vec<_>>().join(" OR ");
        let projection = if fields(kind).contains(&"metadata") {
            "to_jsonb(t) || jsonb_build_object('__metadata_bytes',octet_length(t.metadata::text))"
        } else {
            "to_jsonb(t)"
        };
        let rows: Vec<Value> = sqlx::query_scalar(&format!("SELECT {projection} FROM diraigent.{kind} t WHERE {scope} AND ({eligible}) ORDER BY t.id LIMIT 20"))
            .bind(project).fetch_all(&state.pool).await.map_err(|e| { tracing::error!(entity=kind,error=%e,"Failed to read legacy content batch"); e })?;
        for row in rows {
            let id: Uuid = serde_json::from_value(row["id"].clone()).map_err(error)?;
            for &field in fields(kind) {
                let Some(original) = row.get(field).filter(|v| nonempty(v)) else {
                    continue;
                };
                if FieldRef::parse(original).map_err(error)?.is_some() {
                    continue;
                }
                if field == "metadata" && row["__metadata_bytes"].as_u64().unwrap_or(0) <= 1024 {
                    continue;
                }
                let reference = store.put(project, original).await?;
                let marker = encode_field(kind, field, &reference, original);
                let sql = format!(
                    "UPDATE diraigent.{kind} SET {field}=$2 WHERE id=$1 AND {field} IS NOT DISTINCT FROM $3"
                );
                let affected = if !json_field(kind, field) {
                    sqlx::query(&sql)
                        .bind(id)
                        .bind(marker.as_str())
                        .bind(original.as_str())
                        .execute(&state.pool)
                        .await?
                        .rows_affected()
                } else {
                    sqlx::query(&sql)
                        .bind(id)
                        .bind(&marker)
                        .bind(original)
                        .execute(&state.pool)
                        .await?
                        .rows_affected()
                };
                moved += affected as usize;
            }
        }
    }
    let vectors: Vec<(Uuid,Vec<f64>)> = sqlx::query_as("SELECT id,embedding FROM diraigent.knowledge WHERE project_id=$1 AND embedding IS NOT NULL ORDER BY id LIMIT 20")
        .bind(project).fetch_all(&state.pool).await?;
    for (id, vector) in vectors {
        let reference = store
            .put(project, &serde_json::to_value(&vector).map_err(error)?)
            .await?;
        let mut tx = state.pool.begin().await?;
        let cleared = sqlx::query(
            "UPDATE diraigent.knowledge SET embedding=NULL WHERE id=$1 AND embedding=$2",
        )
        .bind(id)
        .bind(&vector)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if cleared == 1 {
            sqlx::query("INSERT INTO diraigent.knowledge_embedding_ref VALUES ($1,$2,$3,$4) ON CONFLICT(knowledge_id) DO UPDATE SET object_id=EXCLUDED.object_id,checksum=EXCLUDED.checksum,bytes=EXCLUDED.bytes")
                .bind(id).bind(reference.id).bind(reference.checksum).bind(reference.bytes as i64).execute(&mut *tx).await?;
            moved += 1;
        }
        tx.commit().await?;
    }
    Ok(moved)
}
