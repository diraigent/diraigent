//! CryptoDb — transparent encrypt-on-write / decrypt-on-read wrapper.
//!
//! Wraps any `DiraigentDb` and intercepts methods that touch sensitive fields.
//! When the tenant's encryption_mode is "none", all operations pass through unchanged.
//!
//! Encrypted fields and their AAD tags:
//!   integration.credentials   → "integration.credentials"
//!   task.context              → "task.context"
//!   task_update.content       → "task_update.content"
//!   knowledge.content         → "knowledge.content"
//!   decision.context          → "decision.context"
//!   decision.decision         → "decision.decision"
//!   decision.rationale        → "decision.rationale"
//!   webhook.secret            → "webhook.secret"
//!   changed_file.diff         → "changed_file.diff"

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use super::DiraigentDb;
use crate::crypto::{CryptoError, Dek, DekCache};
use crate::error::AppError;
use crate::models::*;

/// A wrapper around a `DiraigentDb` that encrypts/decrypts sensitive fields transparently.
///
/// The `dek_cache` is populated when a user authenticates with a tenant that has
/// encryption enabled. If no DEK is cached for a tenant, operations on encrypted
/// fields will fail with `CryptoError::NotInitialized` (for encrypted tenants)
/// or pass through unchanged (for `encryption_mode = "none"`).
pub struct CryptoDb {
    inner: Arc<dyn DiraigentDb>,
    dek_cache: DekCache,
}

impl CryptoDb {
    pub fn new(inner: Arc<dyn DiraigentDb>, dek_cache: DekCache) -> Self {
        CryptoDb { inner, dek_cache }
    }

    /// Try to get the DEK for a tenant. Returns None if tenant has no encryption.
    async fn dek_for_tenant(&self, tenant_id: Uuid) -> Result<Option<Dek>, AppError> {
        let tenant = self.inner.get_tenant_by_id(tenant_id).await?;
        if tenant.encryption_mode == "none" {
            return Ok(None);
        }
        self.dek_cache
            .get(&tenant_id)
            .await
            .ok_or_else(|| CryptoError::NotInitialized.into())
            .map(Some)
    }

    /// Get DEK for the tenant that owns a project.
    async fn dek_for_project(&self, project_id: Uuid) -> Result<Option<Dek>, AppError> {
        let project = self.inner.get_project_by_id(project_id).await?;
        self.dek_for_tenant(project.tenant_id).await
    }

    /// Get DEK for the tenant that owns a task (via its project).
    async fn dek_for_task(&self, task_id: Uuid) -> Result<Option<Dek>, AppError> {
        let task = self.inner.get_task_by_id(task_id).await?;
        self.dek_for_project(task.project_id).await
    }

    // ── Field-level encrypt/decrypt helpers ──

    fn decrypt_task(dek: &Dek, task: &mut Task) -> Result<(), CryptoError> {
        task.context = dek.decrypt_json(&task.context, "task.context")?;
        Ok(())
    }

    fn decrypt_task_update(dek: &Dek, update: &mut TaskUpdate) -> Result<(), CryptoError> {
        update.content = dek.decrypt_str(&update.content, "task_update.content")?;
        Ok(())
    }

    fn decrypt_knowledge(dek: &Dek, k: &mut Knowledge) -> Result<(), CryptoError> {
        k.content = dek.decrypt_str(&k.content, "knowledge.content")?;
        Ok(())
    }

    fn decrypt_decision(dek: &Dek, d: &mut Decision) -> Result<(), CryptoError> {
        d.context = dek.decrypt_str(&d.context, "decision.context")?;
        if let Some(ref dec) = d.decision {
            d.decision = Some(dek.decrypt_str(dec, "decision.decision")?);
        }
        if let Some(ref rat) = d.rationale {
            d.rationale = Some(dek.decrypt_str(rat, "decision.rationale")?);
        }
        Ok(())
    }

    fn decrypt_integration(dek: &Dek, i: &mut Integration) -> Result<(), CryptoError> {
        i.credentials = dek.decrypt_json(&i.credentials, "integration.credentials")?;
        Ok(())
    }

    fn decrypt_webhook(dek: &Dek, w: &mut Webhook) -> Result<(), CryptoError> {
        if let Some(ref s) = w.secret {
            w.secret = Some(dek.decrypt_str(s, "webhook.secret")?);
        }
        Ok(())
    }

    fn decrypt_changed_file(dek: &Dek, f: &mut ChangedFile) -> Result<(), CryptoError> {
        if let Some(ref d) = f.diff {
            f.diff = Some(dek.decrypt_str(d, "changed_file.diff")?);
        }
        Ok(())
    }
}

/// Macro to delegate a trait method to `self.inner` without any crypto transformation.
macro_rules! delegate {
    ($self:ident, $method:ident $(, $arg:expr)*) => {
        $self.inner.$method($($arg),*).await
    };
}

#[async_trait]
impl DiraigentDb for CryptoDb {
    // ── Health ──
    async fn health_check(&self) -> bool {
        self.inner.health_check().await
    }

    // ── Projects (no encrypted fields) ──
    async fn create_project(
        &self,
        req: &CreateProject,
        owner_id: Uuid,
    ) -> Result<Project, AppError> {
        delegate!(self, create_project, req, owner_id)
    }
    async fn get_project_by_id(&self, id: Uuid) -> Result<Project, AppError> {
        delegate!(self, get_project_by_id, id)
    }
    async fn get_project_by_slug(&self, slug: &str) -> Result<Project, AppError> {
        delegate!(self, get_project_by_slug, slug)
    }
    async fn list_projects(&self, p: &Pagination) -> Result<Vec<Project>, AppError> {
        delegate!(self, list_projects, p)
    }
    async fn list_projects_for_tenant(
        &self,
        tenant_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<Project>, AppError> {
        delegate!(self, list_projects_for_tenant, tenant_id, p)
    }
    async fn update_project(&self, id: Uuid, req: &UpdateProject) -> Result<Project, AppError> {
        delegate!(self, update_project, id, req)
    }
    async fn delete_project(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_project, id)
    }

    // ── Tasks (encrypt context) ──
    async fn create_task(
        &self,
        project_id: Uuid,
        req: &CreateTask,
        created_by: Uuid,
    ) -> Result<Task, AppError> {
        let dek = self.dek_for_project(project_id).await?;
        if let Some(ref dek) = dek {
            // Encrypt the context before writing
            let encrypted_req = CreateTask {
                title: req.title.clone(),
                kind: req.kind.clone(),
                priority: req.priority,
                context: req
                    .context
                    .as_ref()
                    .map(|c| dek.encrypt_json(c, "task.context"))
                    .transpose()?,
                required_capabilities: req.required_capabilities.clone(),
                playbook_id: req.playbook_id,
                decision_id: req.decision_id,
                goal_id: req.goal_id,
            };
            let mut task = self
                .inner
                .create_task(project_id, &encrypted_req, created_by)
                .await?;
            Self::decrypt_task(dek, &mut task)?;
            Ok(task)
        } else {
            self.inner.create_task(project_id, req, created_by).await
        }
    }

    async fn get_task_by_id(&self, task_id: Uuid) -> Result<Task, AppError> {
        let mut task = self.inner.get_task_by_id(task_id).await?;
        if let Some(dek) = self.dek_for_project(task.project_id).await? {
            Self::decrypt_task(&dek, &mut task)?;
        }
        Ok(task)
    }

    async fn list_tasks(
        &self,
        project_id: Uuid,
        filters: &TaskFilters,
    ) -> Result<Vec<Task>, AppError> {
        let mut tasks = self.inner.list_tasks(project_id, filters).await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for task in &mut tasks {
                Self::decrypt_task(&dek, task)?;
            }
        }
        Ok(tasks)
    }

    async fn list_ready_tasks(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError> {
        let mut tasks = self
            .inner
            .list_ready_tasks(project_id, limit, offset)
            .await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for task in &mut tasks {
                Self::decrypt_task(&dek, task)?;
            }
        }
        Ok(tasks)
    }

    async fn update_task(&self, task_id: Uuid, req: &UpdateTask) -> Result<Task, AppError> {
        let dek = self.dek_for_task(task_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = UpdateTask {
                title: req.title.clone(),
                kind: req.kind.clone(),
                priority: req.priority,
                context: req
                    .context
                    .as_ref()
                    .map(|c| dek.encrypt_json(c, "task.context"))
                    .transpose()?,
                required_capabilities: req.required_capabilities.clone(),
                playbook_step: req.playbook_step,
                playbook_id: req.playbook_id,
                flagged: req.flagged,
            };
            let mut task = self.inner.update_task(task_id, &encrypted_req).await?;
            Self::decrypt_task(dek, &mut task)?;
            Ok(task)
        } else {
            self.inner.update_task(task_id, req).await
        }
    }

    async fn transition_task(
        &self,
        task_id: Uuid,
        target_state: &str,
        playbook_step: Option<i32>,
    ) -> Result<Task, AppError> {
        let mut task = self
            .inner
            .transition_task(task_id, target_state, playbook_step)
            .await?;
        if let Some(dek) = self.dek_for_project(task.project_id).await? {
            Self::decrypt_task(&dek, &mut task)?;
        }
        Ok(task)
    }

    async fn claim_task(&self, task_id: Uuid, agent_id: Uuid) -> Result<Task, AppError> {
        let mut task = self.inner.claim_task(task_id, agent_id).await?;
        if let Some(dek) = self.dek_for_project(task.project_id).await? {
            Self::decrypt_task(&dek, &mut task)?;
        }
        Ok(task)
    }

    async fn resolve_claim_step_name(&self, task: &Task) -> Result<String, AppError> {
        delegate!(self, resolve_claim_step_name, task)
    }

    async fn release_task(&self, task_id: Uuid) -> Result<Task, AppError> {
        let mut task = self.inner.release_task(task_id).await?;
        if let Some(dek) = self.dek_for_project(task.project_id).await? {
            Self::decrypt_task(&dek, &mut task)?;
        }
        Ok(task)
    }

    async fn delete_task(&self, task_id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_task, task_id)
    }

    async fn update_task_cost(
        &self,
        task_id: Uuid,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Task, AppError> {
        delegate!(
            self,
            update_task_cost,
            task_id,
            input_tokens,
            output_tokens,
            cost_usd
        )
    }

    // ── Dependencies (no encrypted fields) ──
    async fn add_dependency(
        &self,
        task_id: Uuid,
        depends_on: Uuid,
    ) -> Result<TaskDependency, AppError> {
        delegate!(self, add_dependency, task_id, depends_on)
    }
    async fn remove_dependency(&self, task_id: Uuid, depends_on: Uuid) -> Result<(), AppError> {
        delegate!(self, remove_dependency, task_id, depends_on)
    }
    async fn list_dependencies(&self, task_id: Uuid) -> Result<TaskDependencies, AppError> {
        delegate!(self, list_dependencies, task_id)
    }
    async fn list_blocked_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        delegate!(self, list_blocked_task_ids, project_id)
    }
    async fn list_flagged_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        delegate!(self, list_flagged_task_ids, project_id)
    }
    async fn list_goal_linked_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        delegate!(self, list_goal_linked_task_ids, project_id)
    }

    // ── Task Updates (encrypt content) ──
    async fn create_task_update(
        &self,
        task_id: Uuid,
        req: &CreateTaskUpdate,
        user_id: Option<Uuid>,
    ) -> Result<TaskUpdate, AppError> {
        let dek = self.dek_for_task(task_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = CreateTaskUpdate {
                agent_id: req.agent_id,
                kind: req.kind.clone(),
                content: dek.encrypt_str(&req.content, "task_update.content")?,
                metadata: req.metadata.clone(),
            };
            let mut update = self
                .inner
                .create_task_update(task_id, &encrypted_req, user_id)
                .await?;
            Self::decrypt_task_update(dek, &mut update)?;
            Ok(update)
        } else {
            self.inner.create_task_update(task_id, req, user_id).await
        }
    }

    async fn list_task_updates(
        &self,
        task_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<TaskUpdate>, AppError> {
        let mut updates = self.inner.list_task_updates(task_id, p).await?;
        if let Ok(Some(dek)) = self.dek_for_task(task_id).await {
            for u in &mut updates {
                Self::decrypt_task_update(&dek, u)?;
            }
        }
        Ok(updates)
    }

    // ── Task Comments (no encrypted fields — user-facing, not secrets) ──
    async fn create_task_comment(
        &self,
        task_id: Uuid,
        req: &CreateTaskComment,
        user_id: Option<Uuid>,
    ) -> Result<TaskComment, AppError> {
        delegate!(self, create_task_comment, task_id, req, user_id)
    }
    async fn list_task_comments(
        &self,
        task_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<TaskComment>, AppError> {
        delegate!(self, list_task_comments, task_id, p)
    }

    // ── Agents (no encrypted fields) ──
    async fn register_agent(
        &self,
        req: &CreateAgent,
        owner_id: Uuid,
    ) -> Result<(Agent, String), AppError> {
        delegate!(self, register_agent, req, owner_id)
    }
    async fn authenticate_agent_key(
        &self,
        key_hash: &str,
    ) -> Result<Option<(Uuid, Uuid)>, AppError> {
        delegate!(self, authenticate_agent_key, key_hash)
    }
    async fn get_agent_by_id(&self, id: Uuid) -> Result<Agent, AppError> {
        delegate!(self, get_agent_by_id, id)
    }
    async fn list_agents(&self, p: &Pagination) -> Result<Vec<Agent>, AppError> {
        delegate!(self, list_agents, p)
    }
    async fn update_agent(&self, id: Uuid, req: &UpdateAgent) -> Result<Agent, AppError> {
        delegate!(self, update_agent, id, req)
    }
    async fn agent_heartbeat(&self, id: Uuid, status: Option<&str>) -> Result<Agent, AppError> {
        delegate!(self, agent_heartbeat, id, status)
    }
    async fn list_agent_tasks(
        &self,
        agent_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<Task>, AppError> {
        // These tasks could come from various projects; decrypt each one individually
        let mut tasks = self.inner.list_agent_tasks(agent_id, p).await?;
        for task in &mut tasks {
            if let Ok(Some(dek)) = self.dek_for_project(task.project_id).await {
                Self::decrypt_task(&dek, task)?;
            }
        }
        Ok(tasks)
    }
    async fn verify_agent_owner(&self, agent_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        delegate!(self, verify_agent_owner, agent_id, user_id)
    }

    // ── Goals (no encrypted fields) ──
    async fn create_goal(
        &self,
        project_id: Uuid,
        req: &CreateGoal,
        created_by: Uuid,
    ) -> Result<Goal, AppError> {
        delegate!(self, create_goal, project_id, req, created_by)
    }
    async fn get_goal_by_id(&self, id: Uuid) -> Result<Goal, AppError> {
        delegate!(self, get_goal_by_id, id)
    }
    async fn list_goals(
        &self,
        project_id: Uuid,
        filters: &GoalFilters,
    ) -> Result<Vec<Goal>, AppError> {
        delegate!(self, list_goals, project_id, filters)
    }
    async fn update_goal(&self, id: Uuid, req: &UpdateGoal) -> Result<Goal, AppError> {
        delegate!(self, update_goal, id, req)
    }
    async fn delete_goal(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_goal, id)
    }
    async fn link_task_goal(&self, goal_id: Uuid, task_id: Uuid) -> Result<TaskGoal, AppError> {
        delegate!(self, link_task_goal, goal_id, task_id)
    }
    async fn unlink_task_goal(&self, goal_id: Uuid, task_id: Uuid) -> Result<(), AppError> {
        delegate!(self, unlink_task_goal, goal_id, task_id)
    }
    async fn get_goal_progress(&self, goal_id: Uuid) -> Result<GoalProgress, AppError> {
        delegate!(self, get_goal_progress, goal_id)
    }
    async fn list_goal_tasks(
        &self,
        goal_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError> {
        delegate!(self, list_goal_tasks, goal_id, limit, offset)
    }
    async fn count_goal_tasks(&self, goal_id: Uuid) -> Result<i64, AppError> {
        delegate!(self, count_goal_tasks, goal_id)
    }
    async fn bulk_link_tasks(&self, goal_id: Uuid, task_ids: &[Uuid]) -> Result<i64, AppError> {
        delegate!(self, bulk_link_tasks, goal_id, task_ids)
    }
    async fn get_goal_stats(&self, goal_id: Uuid) -> Result<GoalStats, AppError> {
        delegate!(self, get_goal_stats, goal_id)
    }
    async fn compute_auto_status(&self, goal_id: Uuid) -> Result<Option<String>, AppError> {
        delegate!(self, compute_auto_status, goal_id)
    }
    async fn list_auto_status_goal_ids_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        delegate!(self, list_auto_status_goal_ids_for_task, task_id)
    }
    async fn reorder_goals(
        &self,
        project_id: Uuid,
        goal_ids: &[Uuid],
    ) -> Result<Vec<Goal>, AppError> {
        delegate!(self, reorder_goals, project_id, goal_ids)
    }
    async fn get_goal_ids_for_task(&self, task_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        delegate!(self, get_goal_ids_for_task, task_id)
    }
    async fn get_agent_inherited_goal_ids(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
        exclude_task_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        delegate!(
            self,
            get_agent_inherited_goal_ids,
            agent_id,
            project_id,
            exclude_task_id
        )
    }
    async fn list_goals_for_task(&self, task_id: Uuid) -> Result<Vec<Goal>, AppError> {
        delegate!(self, list_goals_for_task, task_id)
    }
    // ── Goal Comments ──
    async fn create_goal_comment(
        &self,
        goal_id: Uuid,
        req: &CreateGoalComment,
        user_id: Option<Uuid>,
    ) -> Result<GoalComment, AppError> {
        delegate!(self, create_goal_comment, goal_id, req, user_id)
    }
    async fn list_goal_comments(
        &self,
        goal_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<GoalComment>, AppError> {
        delegate!(self, list_goal_comments, goal_id, p)
    }

    // ── Knowledge (encrypt content) ──
    async fn create_knowledge(
        &self,
        project_id: Uuid,
        req: &CreateKnowledge,
        created_by: Uuid,
    ) -> Result<Knowledge, AppError> {
        let dek = self.dek_for_project(project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = CreateKnowledge {
                title: req.title.clone(),
                category: req.category.clone(),
                content: dek.encrypt_str(&req.content, "knowledge.content")?,
                tags: req.tags.clone(),
                metadata: req.metadata.clone(),
            };
            let mut k = self
                .inner
                .create_knowledge(project_id, &encrypted_req, created_by)
                .await?;
            Self::decrypt_knowledge(dek, &mut k)?;
            Ok(k)
        } else {
            self.inner
                .create_knowledge(project_id, req, created_by)
                .await
        }
    }

    async fn get_knowledge_by_id(&self, id: Uuid) -> Result<Knowledge, AppError> {
        let mut k = self.inner.get_knowledge_by_id(id).await?;
        if let Some(dek) = self.dek_for_project(k.project_id).await? {
            Self::decrypt_knowledge(&dek, &mut k)?;
        }
        Ok(k)
    }

    async fn list_knowledge(
        &self,
        project_id: Uuid,
        filters: &KnowledgeFilters,
    ) -> Result<Vec<Knowledge>, AppError> {
        let mut items = self.inner.list_knowledge(project_id, filters).await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for k in &mut items {
                Self::decrypt_knowledge(&dek, k)?;
            }
        }
        Ok(items)
    }

    async fn update_knowledge(
        &self,
        id: Uuid,
        req: &UpdateKnowledge,
    ) -> Result<Knowledge, AppError> {
        let existing = self.inner.get_knowledge_by_id(id).await?;
        let dek = self.dek_for_project(existing.project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = UpdateKnowledge {
                title: req.title.clone(),
                category: req.category.clone(),
                content: req
                    .content
                    .as_ref()
                    .map(|c| dek.encrypt_str(c, "knowledge.content"))
                    .transpose()?,
                tags: req.tags.clone(),
                metadata: req.metadata.clone(),
            };
            let mut k = self.inner.update_knowledge(id, &encrypted_req).await?;
            Self::decrypt_knowledge(dek, &mut k)?;
            Ok(k)
        } else {
            self.inner.update_knowledge(id, req).await
        }
    }

    async fn delete_knowledge(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_knowledge, id)
    }
    async fn count_knowledge(
        &self,
        project_id: Uuid,
        filters: &KnowledgeFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_knowledge, project_id, filters)
    }
    async fn update_knowledge_embedding(
        &self,
        id: Uuid,
        embedding: &[f64],
    ) -> Result<(), AppError> {
        delegate!(self, update_knowledge_embedding, id, embedding)
    }
    async fn list_knowledge_with_embeddings(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Knowledge>, AppError> {
        // Embeddings are stored/read as raw float arrays — no encryption needed.
        // But we still need to decrypt knowledge content for consistency.
        let mut items = self
            .inner
            .list_knowledge_with_embeddings(project_id)
            .await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for k in &mut items {
                Self::decrypt_knowledge(&dek, k)?;
            }
        }
        Ok(items)
    }

    // ── Decisions (encrypt context, decision, rationale) ──
    async fn create_decision(
        &self,
        project_id: Uuid,
        req: &CreateDecision,
        created_by: Uuid,
    ) -> Result<Decision, AppError> {
        let dek = self.dek_for_project(project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = CreateDecision {
                title: req.title.clone(),
                context: dek.encrypt_str(&req.context, "decision.context")?,
                decision: req
                    .decision
                    .as_ref()
                    .map(|d| dek.encrypt_str(d, "decision.decision"))
                    .transpose()?,
                rationale: req
                    .rationale
                    .as_ref()
                    .map(|r| dek.encrypt_str(r, "decision.rationale"))
                    .transpose()?,
                alternatives: req.alternatives.clone(),
                consequences: req.consequences.clone(),
                tags: req.tags.clone(),
            };
            let mut d = self
                .inner
                .create_decision(project_id, &encrypted_req, created_by)
                .await?;
            Self::decrypt_decision(dek, &mut d)?;
            Ok(d)
        } else {
            self.inner
                .create_decision(project_id, req, created_by)
                .await
        }
    }

    async fn get_decision_by_id(&self, id: Uuid) -> Result<Decision, AppError> {
        let mut d = self.inner.get_decision_by_id(id).await?;
        if let Some(dek) = self.dek_for_project(d.project_id).await? {
            Self::decrypt_decision(&dek, &mut d)?;
        }
        Ok(d)
    }

    async fn list_decisions(
        &self,
        project_id: Uuid,
        filters: &DecisionFilters,
    ) -> Result<Vec<Decision>, AppError> {
        let mut items = self.inner.list_decisions(project_id, filters).await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for d in &mut items {
                Self::decrypt_decision(&dek, d)?;
            }
        }
        Ok(items)
    }

    async fn update_decision(&self, id: Uuid, req: &UpdateDecision) -> Result<Decision, AppError> {
        let existing = self.inner.get_decision_by_id(id).await?;
        let dek = self.dek_for_project(existing.project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = UpdateDecision {
                title: req.title.clone(),
                status: req.status.clone(),
                context: req
                    .context
                    .as_ref()
                    .map(|c| dek.encrypt_str(c, "decision.context"))
                    .transpose()?,
                decision: req
                    .decision
                    .as_ref()
                    .map(|d| dek.encrypt_str(d, "decision.decision"))
                    .transpose()?,
                rationale: req
                    .rationale
                    .as_ref()
                    .map(|r| dek.encrypt_str(r, "decision.rationale"))
                    .transpose()?,
                alternatives: req.alternatives.clone(),
                consequences: req.consequences.clone(),
                superseded_by: req.superseded_by,
                decided_by: req.decided_by,
                tags: req.tags.clone(),
            };
            let mut d = self.inner.update_decision(id, &encrypted_req).await?;
            Self::decrypt_decision(dek, &mut d)?;
            Ok(d)
        } else {
            self.inner.update_decision(id, req).await
        }
    }

    async fn delete_decision(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_decision, id)
    }
    async fn count_decisions(
        &self,
        project_id: Uuid,
        filters: &DecisionFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_decisions, project_id, filters)
    }
    async fn list_tasks_by_decision(
        &self,
        decision_id: Uuid,
    ) -> Result<Vec<TaskSummaryForDecision>, AppError> {
        delegate!(self, list_tasks_by_decision, decision_id)
    }

    // ── Observations (no encrypted fields) ──
    async fn create_observation(
        &self,
        project_id: Uuid,
        req: &CreateObservation,
    ) -> Result<Observation, AppError> {
        delegate!(self, create_observation, project_id, req)
    }
    async fn get_observation_by_id(&self, id: Uuid) -> Result<Observation, AppError> {
        delegate!(self, get_observation_by_id, id)
    }
    async fn list_observations(
        &self,
        project_id: Uuid,
        filters: &ObservationFilters,
    ) -> Result<Vec<Observation>, AppError> {
        delegate!(self, list_observations, project_id, filters)
    }
    async fn update_observation(
        &self,
        id: Uuid,
        req: &UpdateObservation,
    ) -> Result<Observation, AppError> {
        delegate!(self, update_observation, id, req)
    }
    async fn dismiss_observation(&self, id: Uuid) -> Result<Observation, AppError> {
        delegate!(self, dismiss_observation, id)
    }
    async fn promote_observation(
        &self,
        obs_id: Uuid,
        req: &PromoteObservation,
        created_by: Uuid,
    ) -> Result<(Observation, Task), AppError> {
        delegate!(self, promote_observation, obs_id, req, created_by)
    }
    async fn count_observations(
        &self,
        project_id: Uuid,
        filters: &ObservationFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_observations, project_id, filters)
    }
    async fn delete_observation(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_observation, id)
    }
    async fn cleanup_observations(
        &self,
        project_id: Uuid,
    ) -> Result<CleanupObservationsResult, AppError> {
        delegate!(self, cleanup_observations, project_id)
    }

    // ── Playbooks (no encrypted fields) ──
    async fn create_playbook(
        &self,
        tenant_id: Uuid,
        req: &CreatePlaybook,
        created_by: Uuid,
    ) -> Result<Playbook, AppError> {
        delegate!(self, create_playbook, tenant_id, req, created_by)
    }
    async fn get_playbook_by_id(&self, id: Uuid) -> Result<Playbook, AppError> {
        delegate!(self, get_playbook_by_id, id)
    }
    async fn list_playbooks(
        &self,
        tenant_id: Uuid,
        filters: &PlaybookFilters,
    ) -> Result<Vec<Playbook>, AppError> {
        delegate!(self, list_playbooks, tenant_id, filters)
    }
    async fn update_playbook(&self, id: Uuid, req: &UpdatePlaybook) -> Result<Playbook, AppError> {
        delegate!(self, update_playbook, id, req)
    }
    async fn fork_playbook(
        &self,
        tenant_id: Uuid,
        source: &Playbook,
        req: &UpdatePlaybook,
        created_by: Uuid,
    ) -> Result<Playbook, AppError> {
        delegate!(self, fork_playbook, tenant_id, source, req, created_by)
    }
    async fn sync_playbook_with_parent(&self, id: Uuid) -> Result<Playbook, AppError> {
        delegate!(self, sync_playbook_with_parent, id)
    }
    async fn delete_playbook(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_playbook, id)
    }

    // ── Step Templates (no encrypted fields) ──
    async fn create_step_template(
        &self,
        tenant_id: Uuid,
        req: &CreateStepTemplate,
        created_by: Uuid,
    ) -> Result<StepTemplate, AppError> {
        delegate!(self, create_step_template, tenant_id, req, created_by)
    }
    async fn get_step_template_by_id(&self, id: Uuid) -> Result<StepTemplate, AppError> {
        delegate!(self, get_step_template_by_id, id)
    }
    async fn list_step_templates(
        &self,
        tenant_id: Uuid,
        filters: &StepTemplateFilters,
    ) -> Result<Vec<StepTemplate>, AppError> {
        delegate!(self, list_step_templates, tenant_id, filters)
    }
    async fn update_step_template(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        req: &UpdateStepTemplate,
    ) -> Result<StepTemplate, AppError> {
        delegate!(self, update_step_template, id, tenant_id, req)
    }
    async fn fork_step_template(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        req: &UpdateStepTemplate,
        created_by: Uuid,
    ) -> Result<StepTemplate, AppError> {
        delegate!(self, fork_step_template, id, tenant_id, req, created_by)
    }
    async fn delete_step_template(&self, id: Uuid, tenant_id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_step_template, id, tenant_id)
    }

    // ── Events (no encrypted fields) ──
    async fn create_event(&self, project_id: Uuid, req: &CreateEvent) -> Result<Event, AppError> {
        delegate!(self, create_event, project_id, req)
    }
    async fn get_event_by_id(&self, id: Uuid) -> Result<Event, AppError> {
        delegate!(self, get_event_by_id, id)
    }
    async fn list_events(
        &self,
        project_id: Uuid,
        filters: &EventFilters,
    ) -> Result<Vec<Event>, AppError> {
        delegate!(self, list_events, project_id, filters)
    }
    async fn list_recent_events(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Event>, AppError> {
        delegate!(self, list_recent_events, project_id, limit)
    }
    async fn count_events(
        &self,
        project_id: Uuid,
        filters: &EventFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_events, project_id, filters)
    }

    // ── Integrations (encrypt credentials) ──
    async fn create_integration(
        &self,
        project_id: Uuid,
        req: &CreateIntegration,
    ) -> Result<Integration, AppError> {
        let dek = self.dek_for_project(project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = CreateIntegration {
                name: req.name.clone(),
                kind: req.kind.clone(),
                provider: req.provider.clone(),
                base_url: req.base_url.clone(),
                auth_type: req.auth_type.clone(),
                credentials: req
                    .credentials
                    .as_ref()
                    .map(|c| dek.encrypt_json(c, "integration.credentials"))
                    .transpose()?,
                config: req.config.clone(),
                capabilities: req.capabilities.clone(),
            };
            let mut i = self
                .inner
                .create_integration(project_id, &encrypted_req)
                .await?;
            Self::decrypt_integration(dek, &mut i)?;
            Ok(i)
        } else {
            self.inner.create_integration(project_id, req).await
        }
    }

    async fn get_integration(&self, id: Uuid) -> Result<Integration, AppError> {
        let mut i = self.inner.get_integration(id).await?;
        if let Some(dek) = self.dek_for_project(i.project_id).await? {
            Self::decrypt_integration(&dek, &mut i)?;
        }
        Ok(i)
    }

    async fn list_integrations(
        &self,
        project_id: Uuid,
        filters: &IntegrationFilters,
    ) -> Result<Vec<Integration>, AppError> {
        let mut items = self.inner.list_integrations(project_id, filters).await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for i in &mut items {
                Self::decrypt_integration(&dek, i)?;
            }
        }
        Ok(items)
    }

    async fn update_integration(
        &self,
        id: Uuid,
        req: &UpdateIntegration,
    ) -> Result<Integration, AppError> {
        let existing = self.inner.get_integration(id).await?;
        let dek = self.dek_for_project(existing.project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = UpdateIntegration {
                name: req.name.clone(),
                kind: req.kind.clone(),
                base_url: req.base_url.clone(),
                auth_type: req.auth_type.clone(),
                credentials: req
                    .credentials
                    .as_ref()
                    .map(|c| dek.encrypt_json(c, "integration.credentials"))
                    .transpose()?,
                config: req.config.clone(),
                capabilities: req.capabilities.clone(),
                enabled: req.enabled,
            };
            let mut i = self.inner.update_integration(id, &encrypted_req).await?;
            Self::decrypt_integration(dek, &mut i)?;
            Ok(i)
        } else {
            self.inner.update_integration(id, req).await
        }
    }

    async fn delete_integration(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_integration, id)
    }
    async fn grant_agent_access(
        &self,
        integration_id: Uuid,
        agent_id: Uuid,
        permissions: Vec<String>,
    ) -> Result<AgentIntegration, AppError> {
        delegate!(
            self,
            grant_agent_access,
            integration_id,
            agent_id,
            permissions
        )
    }
    async fn revoke_agent_access(
        &self,
        integration_id: Uuid,
        agent_id: Uuid,
    ) -> Result<(), AppError> {
        delegate!(self, revoke_agent_access, integration_id, agent_id)
    }
    async fn list_agent_integrations(&self, agent_id: Uuid) -> Result<Vec<Integration>, AppError> {
        // These could span projects; decrypt each individually
        let mut items = self.inner.list_agent_integrations(agent_id).await?;
        for i in &mut items {
            if let Ok(Some(dek)) = self.dek_for_project(i.project_id).await {
                Self::decrypt_integration(&dek, i)?;
            }
        }
        Ok(items)
    }
    async fn list_integration_agents(
        &self,
        integration_id: Uuid,
    ) -> Result<Vec<AgentIntegration>, AppError> {
        delegate!(self, list_integration_agents, integration_id)
    }

    // ── Roles (no encrypted fields) ──
    async fn create_role(&self, tenant_id: Uuid, req: &CreateRole) -> Result<Role, AppError> {
        delegate!(self, create_role, tenant_id, req)
    }
    async fn get_role(&self, id: Uuid) -> Result<Role, AppError> {
        delegate!(self, get_role, id)
    }
    async fn list_roles(&self, tenant_id: Uuid) -> Result<Vec<Role>, AppError> {
        delegate!(self, list_roles, tenant_id)
    }
    async fn update_role(&self, id: Uuid, req: &UpdateRole) -> Result<Role, AppError> {
        delegate!(self, update_role, id, req)
    }
    async fn delete_role(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_role, id)
    }

    // ── Members (no encrypted fields) ──
    async fn create_membership(
        &self,
        tenant_id: Uuid,
        req: &CreateMembership,
    ) -> Result<Membership, AppError> {
        delegate!(self, create_membership, tenant_id, req)
    }
    async fn get_membership(&self, id: Uuid) -> Result<Membership, AppError> {
        delegate!(self, get_membership, id)
    }
    async fn list_members(&self, tenant_id: Uuid) -> Result<Vec<Membership>, AppError> {
        delegate!(self, list_members, tenant_id)
    }
    async fn list_tenant_agent_ids(&self, tenant_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        delegate!(self, list_tenant_agent_ids, tenant_id)
    }
    async fn list_agent_memberships(
        &self,
        agent_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<Membership>, AppError> {
        delegate!(self, list_agent_memberships, agent_id, tenant_id)
    }
    async fn get_active_membership_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Result<Option<Membership>, AppError> {
        delegate!(self, get_active_membership_by_agent, agent_id)
    }
    async fn update_membership(
        &self,
        id: Uuid,
        req: &UpdateMembership,
    ) -> Result<Membership, AppError> {
        delegate!(self, update_membership, id, req)
    }
    async fn remove_membership(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, remove_membership, id)
    }

    // ── Hierarchy (no encrypted fields) ──
    async fn get_project_children(&self, parent_id: Uuid) -> Result<Vec<Project>, AppError> {
        delegate!(self, get_project_children, parent_id)
    }
    async fn get_project_tree(&self, root_id: Uuid) -> Result<Vec<Project>, AppError> {
        delegate!(self, get_project_tree, root_id)
    }

    // ── Delegation (no encrypted fields) ──
    async fn delegate_task(
        &self,
        task_id: Uuid,
        delegated_by: Uuid,
        to_agent_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<Task, AppError> {
        let mut task = self
            .inner
            .delegate_task(task_id, delegated_by, to_agent_id, role_id)
            .await?;
        if let Some(dek) = self.dek_for_project(task.project_id).await? {
            Self::decrypt_task(&dek, &mut task)?;
        }
        Ok(task)
    }

    // ── Auth (no encrypted fields) ──
    async fn check_authority(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
        required_authority: &str,
    ) -> Result<bool, AppError> {
        delegate!(
            self,
            check_authority,
            agent_id,
            project_id,
            required_authority
        )
    }
    async fn check_membership_for_agent(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, AppError> {
        delegate!(self, check_membership_for_agent, agent_id, project_id)
    }
    async fn check_tenant_manage_authority(
        &self,
        agent_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<bool, AppError> {
        delegate!(self, check_tenant_manage_authority, agent_id, tenant_id)
    }

    // ── Audit (no encrypted fields) ──
    async fn create_audit_entry(
        &self,
        project_id: Uuid,
        actor_agent_id: Option<Uuid>,
        actor_user_id: Option<Uuid>,
        action: &str,
        entity_type: &str,
        entity_id: Uuid,
        summary: &str,
        before_state: Option<&serde_json::Value>,
        after_state: Option<&serde_json::Value>,
    ) -> Result<AuditEntry, AppError> {
        self.inner
            .create_audit_entry(
                project_id,
                actor_agent_id,
                actor_user_id,
                action,
                entity_type,
                entity_id,
                summary,
                before_state,
                after_state,
            )
            .await
    }
    async fn list_audit_log(
        &self,
        project_id: Uuid,
        filters: &AuditFilters,
    ) -> Result<Vec<AuditEntry>, AppError> {
        delegate!(self, list_audit_log, project_id, filters)
    }
    async fn get_entity_history(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditEntry>, AppError> {
        delegate!(self, get_entity_history, entity_type, entity_id, limit)
    }
    async fn count_audit_log(
        &self,
        project_id: Uuid,
        filters: &AuditFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_audit_log, project_id, filters)
    }

    // ── Agent Context (decrypt task contexts) ──
    async fn get_agent_context(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<AgentContext>, AppError> {
        let ctx = self.inner.get_agent_context(agent_id, project_id).await?;
        if let Some(mut ctx) = ctx {
            if let Some(dek) = self.dek_for_project(project_id).await? {
                for task in &mut ctx.ready_tasks {
                    Self::decrypt_task(&dek, task)?;
                }
                for task in &mut ctx.my_tasks {
                    Self::decrypt_task(&dek, task)?;
                }
                for k in &mut ctx.knowledge {
                    Self::decrypt_knowledge(&dek, k)?;
                }
                for d in &mut ctx.decisions {
                    Self::decrypt_decision(&dek, d)?;
                }
                for i in &mut ctx.integrations {
                    Self::decrypt_integration(&dek, i)?;
                }
            }
            Ok(Some(ctx))
        } else {
            Ok(None)
        }
    }

    // ── Counts (no encrypted fields) ──
    async fn count_tasks(&self, project_id: Uuid, filters: &TaskFilters) -> Result<i64, AppError> {
        delegate!(self, count_tasks, project_id, filters)
    }

    // ── Webhooks (encrypt secret) ──
    async fn create_webhook(
        &self,
        project_id: Uuid,
        req: &CreateWebhook,
    ) -> Result<Webhook, AppError> {
        let dek = self.dek_for_project(project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = CreateWebhook {
                name: req.name.clone(),
                url: req.url.clone(),
                secret: req
                    .secret
                    .as_ref()
                    .map(|s| dek.encrypt_str(s, "webhook.secret"))
                    .transpose()?,
                events: req.events.clone(),
                metadata: req.metadata.clone(),
            };
            let mut w = self
                .inner
                .create_webhook(project_id, &encrypted_req)
                .await?;
            Self::decrypt_webhook(dek, &mut w)?;
            Ok(w)
        } else {
            self.inner.create_webhook(project_id, req).await
        }
    }

    async fn get_webhook(&self, id: Uuid) -> Result<Webhook, AppError> {
        let mut w = self.inner.get_webhook(id).await?;
        if let Some(dek) = self.dek_for_project(w.project_id).await? {
            Self::decrypt_webhook(&dek, &mut w)?;
        }
        Ok(w)
    }

    async fn list_webhooks(&self, project_id: Uuid) -> Result<Vec<Webhook>, AppError> {
        let mut items = self.inner.list_webhooks(project_id).await?;
        if let Some(dek) = self.dek_for_project(project_id).await? {
            for w in &mut items {
                Self::decrypt_webhook(&dek, w)?;
            }
        }
        Ok(items)
    }

    async fn update_webhook(&self, id: Uuid, req: &UpdateWebhook) -> Result<Webhook, AppError> {
        let existing = self.inner.get_webhook(id).await?;
        let dek = self.dek_for_project(existing.project_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_req = UpdateWebhook {
                name: req.name.clone(),
                url: req.url.clone(),
                secret: req
                    .secret
                    .as_ref()
                    .map(|s| dek.encrypt_str(s, "webhook.secret"))
                    .transpose()?,
                events: req.events.clone(),
                enabled: req.enabled,
                metadata: req.metadata.clone(),
            };
            let mut w = self.inner.update_webhook(id, &encrypted_req).await?;
            Self::decrypt_webhook(dek, &mut w)?;
            Ok(w)
        } else {
            self.inner.update_webhook(id, req).await
        }
    }

    async fn delete_webhook(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_webhook, id)
    }
    async fn list_webhook_deliveries(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDelivery>, AppError> {
        delegate!(self, list_webhook_deliveries, webhook_id, limit)
    }
    async fn list_webhook_dead_letters(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDeadLetter>, AppError> {
        delegate!(self, list_webhook_dead_letters, webhook_id, limit)
    }

    // ── Metrics (no encrypted fields) ──
    async fn get_project_metrics(
        &self,
        project_id: Uuid,
        days: i32,
    ) -> Result<ProjectMetrics, AppError> {
        delegate!(self, get_project_metrics, project_id, days)
    }

    // ── Search (no encrypted fields — searches titles/tags, not encrypted content) ──
    async fn search(
        &self,
        project_id: Uuid,
        query: &str,
        entity_types: Option<&[&str]>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<SearchResult>, i64), AppError> {
        delegate!(self, search, project_id, query, entity_types, limit, offset)
    }

    // ── File Locks (no encrypted fields) ──
    async fn list_file_locks(&self, project_id: Uuid) -> Result<Vec<FileLock>, AppError> {
        delegate!(self, list_file_locks, project_id)
    }
    async fn acquire_file_locks(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        paths: &[String],
        agent_id: Uuid,
    ) -> Result<Vec<FileLock>, AppError> {
        delegate!(
            self,
            acquire_file_locks,
            project_id,
            task_id,
            paths,
            agent_id
        )
    }
    async fn release_file_locks(&self, project_id: Uuid, task_id: Uuid) -> Result<u64, AppError> {
        delegate!(self, release_file_locks, project_id, task_id)
    }
    async fn release_file_locks_for_task(&self, task_id: Uuid) -> Result<u64, AppError> {
        delegate!(self, release_file_locks_for_task, task_id)
    }

    // ── Verifications (no encrypted fields) ──
    async fn create_verification(
        &self,
        project_id: Uuid,
        req: &CreateVerification,
        agent_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> Result<Verification, AppError> {
        delegate!(
            self,
            create_verification,
            project_id,
            req,
            agent_id,
            user_id
        )
    }
    async fn list_verifications(
        &self,
        project_id: Uuid,
        filters: &VerificationFilters,
    ) -> Result<Vec<Verification>, AppError> {
        delegate!(self, list_verifications, project_id, filters)
    }
    async fn count_verifications(
        &self,
        project_id: Uuid,
        filters: &VerificationFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_verifications, project_id, filters)
    }
    async fn get_verification_by_id(&self, id: Uuid) -> Result<Verification, AppError> {
        delegate!(self, get_verification_by_id, id)
    }
    async fn update_verification(
        &self,
        id: Uuid,
        req: &UpdateVerification,
    ) -> Result<Verification, AppError> {
        delegate!(self, update_verification, id, req)
    }

    // ── Changed Files (encrypt diff) ──
    async fn create_changed_files(
        &self,
        task_id: Uuid,
        req: &CreateChangedFiles,
    ) -> Result<Vec<ChangedFileSummary>, AppError> {
        // ChangedFileSummary doesn't include diff, so no decryption needed on response
        // But we need to encrypt the diff on write
        let dek = self.dek_for_task(task_id).await?;
        if let Some(ref dek) = dek {
            let encrypted_files: Vec<CreateChangedFile> = req
                .files
                .iter()
                .map(|f| {
                    Ok(CreateChangedFile {
                        path: f.path.clone(),
                        change_type: f.change_type.clone(),
                        diff: f
                            .diff
                            .as_ref()
                            .map(|d| dek.encrypt_str(d, "changed_file.diff"))
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, CryptoError>>()?;
            let encrypted_req = CreateChangedFiles {
                files: encrypted_files,
            };
            self.inner
                .create_changed_files(task_id, &encrypted_req)
                .await
        } else {
            self.inner.create_changed_files(task_id, req).await
        }
    }

    async fn list_changed_files(&self, task_id: Uuid) -> Result<Vec<ChangedFileSummary>, AppError> {
        // Summary doesn't include diff, no decryption needed
        delegate!(self, list_changed_files, task_id)
    }

    async fn get_changed_file_by_id(&self, id: Uuid) -> Result<ChangedFile, AppError> {
        let mut f = self.inner.get_changed_file_by_id(id).await?;
        // Need to find the task to get the project
        let task = self.inner.get_task_by_id(f.task_id).await?;
        if let Some(dek) = self.dek_for_project(task.project_id).await? {
            Self::decrypt_changed_file(&dek, &mut f)?;
        }
        Ok(f)
    }

    // ── Stale detector (internal) ──
    async fn query_stale_tasks(&self, default_timeout: i64) -> anyhow::Result<Vec<StaleTaskInfo>> {
        self.inner.query_stale_tasks(default_timeout).await
    }
    async fn release_stale_task_conditional(&self, task_id: Uuid) -> anyhow::Result<bool> {
        self.inner.release_stale_task_conditional(task_id).await
    }
    async fn mark_agent_offline(&self, agent_id: Uuid) -> anyhow::Result<()> {
        self.inner.mark_agent_offline(agent_id).await
    }
    async fn get_project_timeout(&self, project_id: Uuid) -> i64 {
        self.inner.get_project_timeout(project_id).await
    }
    async fn mark_inactive_agents_offline(
        &self,
        threshold_seconds: i64,
    ) -> anyhow::Result<Vec<Uuid>> {
        self.inner
            .mark_inactive_agents_offline(threshold_seconds)
            .await
    }
    async fn revoke_stale_agents(&self, threshold_days: i64) -> anyhow::Result<Vec<Uuid>> {
        self.inner.revoke_stale_agents(threshold_days).await
    }

    // ── Auth User ──
    async fn resolve_or_create_user(&self, auth_user_id: &str) -> Result<Uuid, AppError> {
        self.inner.resolve_or_create_user(auth_user_id).await
    }

    async fn ensure_dev_user(&self, user_id: Uuid) -> Result<(), AppError> {
        self.inner.ensure_dev_user(user_id).await
    }

    // ── Webhook dispatch (internal) ──
    async fn list_webhooks_enabled(&self, project_id: Uuid) -> anyhow::Result<Vec<Webhook>> {
        self.inner.list_webhooks_enabled(project_id).await
    }
    async fn record_webhook_delivery(
        &self,
        webhook_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        status: Option<i32>,
        response_body: Option<&str>,
        success: bool,
        attempt: i32,
    ) {
        self.inner
            .record_webhook_delivery(
                webhook_id,
                event_type,
                payload,
                status,
                response_body,
                success,
                attempt,
            )
            .await
    }
    async fn record_webhook_dead_letter(
        &self,
        webhook_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        last_status: Option<i32>,
        last_body: Option<&str>,
        attempts: i32,
    ) {
        self.inner
            .record_webhook_dead_letter(
                webhook_id,
                event_type,
                payload,
                last_status,
                last_body,
                attempts,
            )
            .await
    }

    // ── Packages (no encrypted fields) ──
    async fn get_package_for_project(&self, project_id: Uuid) -> Result<Option<Package>, AppError> {
        delegate!(self, get_package_for_project, project_id)
    }
    async fn list_packages(&self) -> Result<Vec<Package>, AppError> {
        delegate!(self, list_packages)
    }
    async fn get_package_by_id(&self, id: Uuid) -> Result<Package, AppError> {
        delegate!(self, get_package_by_id, id)
    }
    async fn get_package_by_slug(&self, slug: &str) -> Result<Package, AppError> {
        delegate!(self, get_package_by_slug, slug)
    }
    async fn create_package(&self, req: &CreatePackage) -> Result<Package, AppError> {
        delegate!(self, create_package, req)
    }
    async fn update_package(&self, id: Uuid, req: &UpdatePackage) -> Result<Package, AppError> {
        delegate!(self, update_package, id, req)
    }
    async fn delete_package(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_package, id)
    }

    // ── Tenants (no encrypted fields — these manage the encryption config itself) ──
    async fn create_tenant(&self, req: &CreateTenant) -> Result<Tenant, AppError> {
        delegate!(self, create_tenant, req)
    }
    async fn get_tenant_by_id(&self, id: Uuid) -> Result<Tenant, AppError> {
        delegate!(self, get_tenant_by_id, id)
    }
    async fn get_tenant_by_slug(&self, slug: &str) -> Result<Tenant, AppError> {
        delegate!(self, get_tenant_by_slug, slug)
    }
    async fn list_tenants(&self, filters: &TenantFilters) -> Result<Vec<Tenant>, AppError> {
        delegate!(self, list_tenants, filters)
    }
    async fn update_tenant(&self, id: Uuid, req: &UpdateTenant) -> Result<Tenant, AppError> {
        delegate!(self, update_tenant, id, req)
    }
    async fn delete_tenant(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_tenant, id)
    }
    async fn add_tenant_member(
        &self,
        tenant_id: Uuid,
        req: &AddTenantMember,
    ) -> Result<TenantMember, AppError> {
        delegate!(self, add_tenant_member, tenant_id, req)
    }
    async fn list_tenant_members(&self, tenant_id: Uuid) -> Result<Vec<TenantMember>, AppError> {
        delegate!(self, list_tenant_members, tenant_id)
    }
    async fn update_tenant_member(
        &self,
        member_id: Uuid,
        req: &UpdateTenantMember,
    ) -> Result<TenantMember, AppError> {
        delegate!(self, update_tenant_member, member_id, req)
    }
    async fn remove_tenant_member(&self, member_id: Uuid) -> Result<(), AppError> {
        delegate!(self, remove_tenant_member, member_id)
    }
    async fn get_tenant_member_for_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TenantMember>, AppError> {
        delegate!(self, get_tenant_member_for_user, tenant_id, user_id)
    }
    async fn get_tenant_for_user(&self, user_id: Uuid) -> Result<Option<Tenant>, AppError> {
        delegate!(self, get_tenant_for_user, user_id)
    }
    async fn create_wrapped_key(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        req: &CreateWrappedKey,
    ) -> Result<WrappedKey, AppError> {
        delegate!(self, create_wrapped_key, tenant_id, user_id, req)
    }
    async fn list_wrapped_keys(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<WrappedKey>, AppError> {
        delegate!(self, list_wrapped_keys, tenant_id, user_id)
    }
    async fn delete_wrapped_key(&self, key_id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_wrapped_key, key_id)
    }

    // ── Reports (no encrypted fields, plain delegation) ──────────────────────
    async fn create_report(
        &self,
        project_id: Uuid,
        req: &CreateReport,
        created_by: Uuid,
    ) -> Result<Report, AppError> {
        delegate!(self, create_report, project_id, req, created_by)
    }
    async fn get_report_by_id(&self, id: Uuid) -> Result<Report, AppError> {
        delegate!(self, get_report_by_id, id)
    }
    async fn list_reports(
        &self,
        project_id: Uuid,
        filters: &ReportFilters,
    ) -> Result<Vec<Report>, AppError> {
        delegate!(self, list_reports, project_id, filters)
    }
    async fn count_reports(
        &self,
        project_id: Uuid,
        filters: &ReportFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_reports, project_id, filters)
    }
    async fn update_report(&self, id: Uuid, req: &UpdateReport) -> Result<Report, AppError> {
        delegate!(self, update_report, id, req)
    }
    async fn get_report_by_task_id(&self, task_id: Uuid) -> Result<Option<Report>, AppError> {
        delegate!(self, get_report_by_task_id, task_id)
    }
    async fn delete_report(&self, id: Uuid) -> Result<(), AppError> {
        delegate!(self, delete_report, id)
    }

    // ── Task Logs (no encrypted fields, plain delegation) ─────────────────────
    async fn create_task_log(
        &self,
        project_id: Uuid,
        agent_id: Option<Uuid>,
        req: &CreateTaskLog,
    ) -> Result<TaskLog, AppError> {
        delegate!(self, create_task_log, project_id, agent_id, req)
    }
    async fn list_task_logs(
        &self,
        project_id: Uuid,
        filters: &TaskLogFilters,
    ) -> Result<Vec<TaskLogSummary>, AppError> {
        delegate!(self, list_task_logs, project_id, filters)
    }
    async fn count_task_logs(
        &self,
        project_id: Uuid,
        filters: &TaskLogFilters,
    ) -> Result<i64, AppError> {
        delegate!(self, count_task_logs, project_id, filters)
    }
    async fn get_task_log_by_id(&self, id: Uuid) -> Result<TaskLog, AppError> {
        delegate!(self, get_task_log_by_id, id)
    }
}
