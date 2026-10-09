//! A project-content interface with two adapters: existing central storage and
//! an explicitly selected Orchestra. Central rows become indexes after transfer.
use crate::{AppState, error::AppError, models::*, ws_protocol::WsMessage};
use diraigent_types::project_content::*;
use serde_json::{Value, json};
use uuid::Uuid;

pub async fn owner(state: &AppState, project: Uuid) -> Result<Option<Uuid>, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT agent_id FROM diraigent.project_content_owner WHERE project_id=$1",
    )
    .bind(project)
    .fetch_optional(&state.pool)
    .await?)
}
struct Pending {
    registry: std::sync::Arc<crate::ws_registry::WsRegistry>,
    id: String,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.registry.remove_content_request(&self.id);
    }
}
pub async fn store_id(state: &AppState, project: Uuid) -> Result<Option<Uuid>, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT store_id FROM diraigent.project_content_owner WHERE project_id=$1",
    )
    .bind(project)
    .fetch_optional(&state.pool)
    .await?)
}
pub async fn request(
    state: &AppState,
    project: Uuid,
    agent: Uuid,
    request: ContentRequest,
) -> Result<Value, AppError> {
    require_active_owner(state, project, agent).await?;
    let expected_store_id = store_id(state, project).await?;
    let id = Uuid::now_v7().to_string();
    let rx = state
        .ws_registry
        .register_content_request(id.clone(), agent);
    let _pending = Pending {
        registry: state.ws_registry.clone(),
        id: id.clone(),
    };
    if !state.ws_registry.send_to_agent(
        agent,
        WsMessage::ContentRequest {
            request_id: id,
            project_id: project,
            request,
            expected_store_id,
        },
    ) {
        return Err(AppError::ServiceUnavailable(
            "Project storage owner is offline".into(),
        ));
    }
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), rx)
        .await
        .map_err(|_| AppError::ServiceUnavailable("Project storage request timed out".into()))?
        .map_err(|_| AppError::ServiceUnavailable("Project storage owner disconnected".into()))?;
    result.map_err(|e| match e {
        ContentError::NotFound => AppError::NotFound("Project content not found".into()),
        ContentError::Conflict => AppError::Conflict(
            "Project content changed or conversation is busy; reload before retrying".into(),
        ),
        ContentError::Invalid => {
            AppError::Validation("Invalid or oversized project content".into())
        }
        ContentError::Unavailable => {
            AppError::ServiceUnavailable("Project storage unavailable".into())
        }
    })
}
pub async fn require_active_owner(
    state: &AppState,
    project: Uuid,
    agent: Uuid,
) -> Result<(), AppError> {
    let active: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM diraigent.agent a JOIN diraigent.membership m ON m.agent_id=a.id AND m.status='active' JOIN diraigent.project p ON p.tenant_id=m.tenant_id WHERE a.id=$1 AND p.id=$2 AND a.status<>'revoked')")
        .bind(agent).bind(project).fetch_one(&state.pool).await?;
    if !active {
        return Err(AppError::ServiceUnavailable(
            "Project storage owner no longer has workspace access".into(),
        ));
    }
    Ok(())
}
pub async fn get(state: &AppState, kind: ContentKind, id: Uuid) -> Result<Option<Value>, AppError> {
    let project: Option<Uuid> = sqlx::query_scalar(
        "SELECT project_id FROM diraigent.project_content_ref WHERE kind=$1 AND object_id=$2",
    )
    .bind(kind.as_str())
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    match project {
        None => Ok(None),
        Some(p) => {
            let agent = owner(state, p).await?.ok_or_else(|| {
                AppError::ServiceUnavailable("Project storage owner missing".into())
            })?;
            Ok(Some(
                request(state, p, agent, ContentRequest::Get { kind, id }).await?,
            ))
        }
    }
}
async fn put(
    state: &AppState,
    project: Uuid,
    kind: ContentKind,
    id: Uuid,
    value: Value,
) -> Result<(), AppError> {
    let agent = owner(state, project)
        .await?
        .ok_or_else(|| AppError::Validation("No project storage owner configured".into()))?;
    request(
        state,
        project,
        agent,
        ContentRequest::Put { kind, id, value },
    )
    .await?;
    Ok(())
}
async fn index(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project: Uuid,
    kind: ContentKind,
    id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO diraigent.project_content_ref VALUES ($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(kind.as_str())
    .bind(id)
    .bind(project)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn create_log(
    state: &AppState,
    project: Uuid,
    agent: Option<Uuid>,
    req: &CreateTaskLog,
) -> Result<TaskLog, AppError> {
    if state.db.get_task_by_id(req.task_id).await?.project_id != project {
        return Err(AppError::Validation(
            "Task belongs to another project".into(),
        ));
    }
    if owner(state, project).await?.is_none() {
        return state.db.create_task_log(project, agent, req).await;
    }
    let id = Uuid::now_v7();
    let value = json!({"content":req.content,"metadata":req.metadata.clone().unwrap_or(json!({}))});
    put(state, project, ContentKind::Log, id, value.clone()).await?;
    let mut tx = state.pool.begin().await?;
    let mut log=sqlx::query_as::<_,TaskLog>("INSERT INTO diraigent.task_log (id,project_id,task_id,agent_id,step_name,content,metadata) VALUES ($1,$2,$3,$4,$5,'','{}') RETURNING *")
        .bind(id).bind(project).bind(req.task_id).bind(agent).bind(req.step_name.as_deref().unwrap_or("working")).fetch_one(&mut *tx).await?;
    index(&mut tx, project, ContentKind::Log, id).await?;
    tx.commit().await?;
    log.content = req.content.clone();
    log.metadata = value["metadata"].clone();
    Ok(log)
}
pub async fn create_files(
    state: &AppState,
    task_id: Uuid,
    req: &CreateChangedFiles,
) -> Result<Vec<ChangedFileSummary>, AppError> {
    let project = state.db.get_task_by_id(task_id).await?.project_id;
    if owner(state, project).await?.is_none() {
        return state.db.create_changed_files(task_id, req).await;
    }
    let mut payloads = Vec::new();
    for f in &req.files {
        let id = Uuid::now_v7();
        put(
            state,
            project,
            ContentKind::Diff,
            id,
            json!({"diff":f.diff}),
        )
        .await?;
        payloads.push((id, f));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM diraigent.project_content_ref WHERE kind='diff' AND object_id IN (SELECT id FROM diraigent.task_changed_file WHERE task_id=$1)").bind(task_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM diraigent.task_changed_file WHERE task_id=$1")
        .bind(task_id)
        .execute(&mut *tx)
        .await?;
    let mut rows = Vec::new();
    for (id, f) in payloads {
        let row=sqlx::query_as::<_,ChangedFileSummary>("INSERT INTO diraigent.task_changed_file (id,task_id,path,change_type,diff) VALUES ($1,$2,$3,$4,NULL) RETURNING id,task_id,path,change_type,created_at")
            .bind(id).bind(task_id).bind(&f.path).bind(&f.change_type).fetch_one(&mut *tx).await?;
        index(&mut tx, project, ContentKind::Diff, id).await?;
        rows.push(row);
    }
    tx.commit().await?;
    Ok(rows)
}
pub async fn create_update(
    state: &AppState,
    task_id: Uuid,
    req: &CreateTaskUpdate,
    user: Option<Uuid>,
) -> Result<TaskUpdate, AppError> {
    let project = state.db.get_task_by_id(task_id).await?.project_id;
    if req.kind.as_deref() != Some("artifact") || owner(state, project).await?.is_none() {
        return state.db.create_task_update(task_id, req, user).await;
    }
    let id = Uuid::now_v7();
    put(
        state,
        project,
        ContentKind::Artifact,
        id,
        json!({"content":req.content,"metadata":req.metadata.clone().unwrap_or(json!({}))}),
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    let mut update=sqlx::query_as::<_,TaskUpdate>("INSERT INTO diraigent.task_update (id,task_id,agent_id,user_id,kind,content,metadata) VALUES ($1,$2,$3,$4,'artifact','','{}') RETURNING *")
        .bind(id).bind(task_id).bind(req.agent_id).bind(user).fetch_one(&mut *tx).await?;
    index(&mut tx, project, ContentKind::Artifact, id).await?;
    tx.commit().await?;
    update.content = req.content.clone();
    update.metadata = req.metadata.clone().unwrap_or(json!({}));
    Ok(update)
}
pub async fn hydrate_updates(state: &AppState, updates: &mut [TaskUpdate]) -> Result<(), AppError> {
    for update in updates {
        if let Some(v) = get(state, ContentKind::Artifact, update.id).await? {
            update.content = v["content"].as_str().unwrap_or_default().into();
            update.metadata = v["metadata"].clone();
        }
    }
    Ok(())
}

/// Explicit, bounded migration: read decrypted legacy data, transfer and verify,
/// then atomically replace payloads with routing references. Never edit history
/// migrations or purge backups. Retry after a disconnect safely.
pub async fn migrate(state: &AppState, project: Uuid) -> Result<Value, AppError> {
    let mut moved = 0;
    for kind in [ContentKind::Log, ContentKind::Diff, ContentKind::Artifact] {
        let query = match kind {
            ContentKind::Log => {
                "SELECT l.id FROM diraigent.task_log l WHERE l.project_id=$1 AND NOT EXISTS(SELECT 1 FROM diraigent.project_content_ref r WHERE r.object_id=l.id AND r.kind='log') ORDER BY l.id LIMIT 20"
            }
            ContentKind::Diff => {
                "SELECT f.id FROM diraigent.task_changed_file f JOIN diraigent.task t ON t.id=f.task_id WHERE t.project_id=$1 AND NOT EXISTS(SELECT 1 FROM diraigent.project_content_ref r WHERE r.object_id=f.id AND r.kind='diff') ORDER BY f.id LIMIT 20"
            }
            ContentKind::Artifact => {
                "SELECT u.id FROM diraigent.task_update u JOIN diraigent.task t ON t.id=u.task_id WHERE t.project_id=$1 AND u.kind='artifact' AND NOT EXISTS(SELECT 1 FROM diraigent.project_content_ref r WHERE r.object_id=u.id AND r.kind='artifact') ORDER BY u.id LIMIT 20"
            }
        };
        let ids: Vec<Uuid> = sqlx::query_scalar(query)
            .bind(project)
            .fetch_all(&state.pool)
            .await?;
        for id in ids {
            let value = match kind {
                ContentKind::Log => {
                    let l = state.db.get_task_log_by_id(id).await?;
                    json!({"content":l.content,"metadata":l.metadata})
                }
                ContentKind::Diff => {
                    let f = state.db.get_changed_file_by_id(id).await?;
                    json!({"diff":f.diff})
                }
                ContentKind::Artifact => {
                    let task: Uuid =
                        sqlx::query_scalar("SELECT task_id FROM diraigent.task_update WHERE id=$1")
                            .bind(id)
                            .fetch_one(&state.pool)
                            .await?;
                    // Export through the decryption adapter, with bounded pagination.
                    let mut offset = 0;
                    let update = loop {
                        let page = state
                            .db
                            .list_task_updates(
                                task,
                                &Pagination {
                                    limit: Some(100),
                                    offset: Some(offset),
                                },
                            )
                            .await?;
                        if let Some(u) = page.iter().find(|u| u.id == id) {
                            break u.clone();
                        }
                        if page.is_empty() {
                            return Err(AppError::NotFound("Artifact no longer exists".into()));
                        }
                        offset += 100;
                    };
                    json!({"content":update.content,"metadata":update.metadata})
                }
            };
            put(state, project, kind, id, value.clone()).await?;
            let agent = owner(state, project)
                .await?
                .ok_or_else(|| AppError::Validation("Owner missing".into()))?;
            if request(state, project, agent, ContentRequest::Get { kind, id }).await? != value {
                return Err(AppError::ServiceUnavailable(
                    "Transferred content verification failed".into(),
                ));
            }
            let mut tx = state.pool.begin().await?;
            let clear = match kind {
                ContentKind::Log => {
                    "UPDATE diraigent.task_log SET content='',metadata='{}' WHERE id=$1"
                }
                ContentKind::Diff => "UPDATE diraigent.task_changed_file SET diff=NULL WHERE id=$1",
                ContentKind::Artifact => {
                    "UPDATE diraigent.task_update SET content='',metadata='{}' WHERE id=$1"
                }
            };
            sqlx::query(clear).bind(id).execute(&mut *tx).await?;
            index(&mut tx, project, kind, id).await?;
            tx.commit().await?;
            moved += 1;
        }
    }
    Ok(json!({"moved":moved,"batch_limit_per_kind":20}))
}
