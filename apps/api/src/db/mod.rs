//! Database abstraction layer.
//!
//! [`DiraigentDb`] is an async trait that abstracts over all database operations.
//! [`PostgresDb`] is the sole implementation, backed by a `PgPool`.

pub mod crypto;
pub mod postgres;

pub use crypto::CryptoDb;
pub use postgres::PostgresDb;

use async_trait::async_trait;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::*;

/// The complete database interface for Diraigent.
///
/// All business-logic calls go through this trait so that route handlers,
/// auth helpers, and background workers are independent of the concrete DB.
#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait DiraigentDb: Send + Sync {
    // ── Health ────────────────────────────────────────────────────────────────
    async fn health_check(&self) -> bool;

    // ── Projects ──────────────────────────────────────────────────────────────
    async fn create_project(
        &self,
        req: &CreateProject,
        owner_id: Uuid,
    ) -> Result<Project, AppError>;
    async fn get_project_by_id(&self, id: Uuid) -> Result<Project, AppError>;
    async fn get_project_by_slug(&self, slug: &str) -> Result<Project, AppError>;
    async fn list_projects(&self, p: &Pagination) -> Result<Vec<Project>, AppError>;
    async fn list_projects_for_tenant(
        &self,
        tenant_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<Project>, AppError>;
    async fn update_project(&self, id: Uuid, req: &UpdateProject) -> Result<Project, AppError>;
    async fn delete_project(&self, id: Uuid) -> Result<(), AppError>;

    // ── Tasks ─────────────────────────────────────────────────────────────────
    async fn create_task(
        &self,
        project_id: Uuid,
        req: &CreateTask,
        created_by: Uuid,
    ) -> Result<Task, AppError>;
    async fn get_task_by_id(&self, task_id: Uuid) -> Result<Task, AppError>;
    async fn list_tasks(
        &self,
        project_id: Uuid,
        filters: &TaskFilters,
    ) -> Result<Vec<Task>, AppError>;
    async fn list_ready_tasks(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError>;
    async fn update_task(&self, task_id: Uuid, req: &UpdateTask) -> Result<Task, AppError>;
    async fn transition_task(
        &self,
        task_id: Uuid,
        target_state: &str,
        playbook_step: Option<i32>,
    ) -> Result<Task, AppError>;
    async fn claim_task(&self, task_id: Uuid, agent_id: Uuid) -> Result<Task, AppError>;
    async fn resolve_claim_step_name(&self, task: &Task) -> Result<String, AppError>;
    async fn release_task(&self, task_id: Uuid) -> Result<Task, AppError>;
    async fn delete_task(&self, task_id: Uuid) -> Result<(), AppError>;

    // ── Task Cost Metrics ─────────────────────────────────────────────────────
    async fn update_task_cost(
        &self,
        task_id: Uuid,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Task, AppError>;

    // ── Dependencies ──────────────────────────────────────────────────────────
    async fn add_dependency(
        &self,
        task_id: Uuid,
        depends_on: Uuid,
    ) -> Result<TaskDependency, AppError>;
    async fn remove_dependency(&self, task_id: Uuid, depends_on: Uuid) -> Result<(), AppError>;
    async fn list_dependencies(&self, task_id: Uuid) -> Result<TaskDependencies, AppError>;
    async fn list_blocked_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    async fn list_flagged_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    async fn list_goal_linked_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    async fn list_tasks_with_blocker_updates(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Task>, AppError>;

    // ── Task Updates ──────────────────────────────────────────────────────────
    async fn create_task_update(
        &self,
        task_id: Uuid,
        req: &CreateTaskUpdate,
        user_id: Option<Uuid>,
    ) -> Result<TaskUpdate, AppError>;
    async fn list_task_updates(
        &self,
        task_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<TaskUpdate>, AppError>;

    // ── Task Comments ─────────────────────────────────────────────────────────
    async fn create_task_comment(
        &self,
        task_id: Uuid,
        req: &CreateTaskComment,
        user_id: Option<Uuid>,
    ) -> Result<TaskComment, AppError>;
    async fn list_task_comments(
        &self,
        task_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<TaskComment>, AppError>;

    // ── Agents ────────────────────────────────────────────────────────────────
    async fn register_agent(
        &self,
        req: &CreateAgent,
        owner_id: Uuid,
    ) -> Result<(Agent, String), AppError>;
    /// Authenticate an agent API key (dak_...). Returns (agent_id, owner_id) if valid.
    async fn authenticate_agent_key(
        &self,
        key_hash: &str,
    ) -> Result<Option<(Uuid, Uuid)>, AppError>;
    async fn get_agent_by_id(&self, id: Uuid) -> Result<Agent, AppError>;
    async fn list_agents(&self, p: &Pagination) -> Result<Vec<Agent>, AppError>;
    async fn update_agent(&self, id: Uuid, req: &UpdateAgent) -> Result<Agent, AppError>;
    async fn agent_heartbeat(&self, id: Uuid, status: Option<&str>) -> Result<Agent, AppError>;
    async fn list_agent_tasks(&self, agent_id: Uuid, p: &Pagination)
    -> Result<Vec<Task>, AppError>;
    /// Returns true if the agent exists and its owner_id matches the given user_id,
    /// OR if the agent has no owner_id set (legacy agents created before ownership tracking).
    async fn verify_agent_owner(&self, agent_id: Uuid, user_id: Uuid) -> Result<bool, AppError>;

    // ── Goals ─────────────────────────────────────────────────────────────────
    async fn create_goal(
        &self,
        project_id: Uuid,
        req: &CreateGoal,
        created_by: Uuid,
    ) -> Result<Goal, AppError>;
    async fn get_goal_by_id(&self, id: Uuid) -> Result<Goal, AppError>;
    async fn list_goals(
        &self,
        project_id: Uuid,
        filters: &GoalFilters,
    ) -> Result<Vec<Goal>, AppError>;
    async fn update_goal(&self, id: Uuid, req: &UpdateGoal) -> Result<Goal, AppError>;
    async fn delete_goal(&self, id: Uuid) -> Result<(), AppError>;
    async fn link_task_goal(&self, goal_id: Uuid, task_id: Uuid) -> Result<TaskGoal, AppError>;
    async fn unlink_task_goal(&self, goal_id: Uuid, task_id: Uuid) -> Result<(), AppError>;
    async fn get_goal_progress(&self, goal_id: Uuid) -> Result<GoalProgress, AppError>;
    async fn list_goal_tasks(
        &self,
        goal_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError>;
    async fn count_goal_tasks(&self, goal_id: Uuid) -> Result<i64, AppError>;
    async fn bulk_link_tasks(&self, goal_id: Uuid, task_ids: &[Uuid]) -> Result<i64, AppError>;
    async fn get_goal_stats(&self, goal_id: Uuid) -> Result<GoalStats, AppError>;
    async fn compute_auto_status(&self, goal_id: Uuid) -> Result<Option<String>, AppError>;
    async fn list_auto_status_goal_ids_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError>;
    async fn reorder_goals(
        &self,
        project_id: Uuid,
        goal_ids: &[Uuid],
    ) -> Result<Vec<Goal>, AppError>;
    /// Return all goal IDs linked to a task (no auto_status filter).
    async fn get_goal_ids_for_task(&self, task_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    /// Return distinct goal IDs inherited from an agent's active tasks in a project.
    async fn get_agent_inherited_goal_ids(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
        exclude_task_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError>;
    async fn list_goals_for_task(&self, task_id: Uuid) -> Result<Vec<Goal>, AppError>;

    // ── Goal Comments ──────────────────────────────────────────────────────────
    async fn create_goal_comment(
        &self,
        goal_id: Uuid,
        req: &CreateGoalComment,
        user_id: Option<Uuid>,
    ) -> Result<GoalComment, AppError>;
    async fn list_goal_comments(
        &self,
        goal_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<GoalComment>, AppError>;

    // ── Knowledge ─────────────────────────────────────────────────────────────
    async fn create_knowledge(
        &self,
        project_id: Uuid,
        req: &CreateKnowledge,
        created_by: Uuid,
    ) -> Result<Knowledge, AppError>;
    async fn get_knowledge_by_id(&self, id: Uuid) -> Result<Knowledge, AppError>;
    async fn list_knowledge(
        &self,
        project_id: Uuid,
        filters: &KnowledgeFilters,
    ) -> Result<Vec<Knowledge>, AppError>;
    async fn update_knowledge(
        &self,
        id: Uuid,
        req: &UpdateKnowledge,
    ) -> Result<Knowledge, AppError>;
    async fn delete_knowledge(&self, id: Uuid) -> Result<(), AppError>;
    async fn count_knowledge(
        &self,
        project_id: Uuid,
        filters: &KnowledgeFilters,
    ) -> Result<i64, AppError>;
    /// Store or update the embedding vector for a knowledge entry.
    async fn update_knowledge_embedding(&self, id: Uuid, embedding: &[f64])
    -> Result<(), AppError>;
    /// Fetch all knowledge entries for a project that have embeddings.
    /// Used for in-process cosine similarity ranking.
    async fn list_knowledge_with_embeddings(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Knowledge>, AppError>;

    // ── Decisions ─────────────────────────────────────────────────────────────
    async fn create_decision(
        &self,
        project_id: Uuid,
        req: &CreateDecision,
        created_by: Uuid,
    ) -> Result<Decision, AppError>;
    async fn get_decision_by_id(&self, id: Uuid) -> Result<Decision, AppError>;
    async fn list_decisions(
        &self,
        project_id: Uuid,
        filters: &DecisionFilters,
    ) -> Result<Vec<Decision>, AppError>;
    async fn update_decision(&self, id: Uuid, req: &UpdateDecision) -> Result<Decision, AppError>;
    async fn delete_decision(&self, id: Uuid) -> Result<(), AppError>;
    async fn count_decisions(
        &self,
        project_id: Uuid,
        filters: &DecisionFilters,
    ) -> Result<i64, AppError>;
    async fn list_tasks_by_decision(
        &self,
        decision_id: Uuid,
    ) -> Result<Vec<TaskSummaryForDecision>, AppError>;

    // ── Observations ──────────────────────────────────────────────────────────
    async fn create_observation(
        &self,
        project_id: Uuid,
        req: &CreateObservation,
    ) -> Result<Observation, AppError>;
    async fn get_observation_by_id(&self, id: Uuid) -> Result<Observation, AppError>;
    async fn list_observations(
        &self,
        project_id: Uuid,
        filters: &ObservationFilters,
    ) -> Result<Vec<Observation>, AppError>;
    async fn update_observation(
        &self,
        id: Uuid,
        req: &UpdateObservation,
    ) -> Result<Observation, AppError>;
    async fn dismiss_observation(&self, id: Uuid) -> Result<Observation, AppError>;
    async fn promote_observation(
        &self,
        obs_id: Uuid,
        req: &PromoteObservation,
        created_by: Uuid,
    ) -> Result<(Observation, Task), AppError>;
    async fn count_observations(
        &self,
        project_id: Uuid,
        filters: &ObservationFilters,
    ) -> Result<i64, AppError>;
    async fn delete_observation(&self, id: Uuid) -> Result<(), AppError>;
    async fn cleanup_observations(
        &self,
        project_id: Uuid,
    ) -> Result<CleanupObservationsResult, AppError>;

    // ── Playbooks ─────────────────────────────────────────────────────────────
    async fn create_playbook(
        &self,
        tenant_id: Uuid,
        req: &CreatePlaybook,
        created_by: Uuid,
    ) -> Result<Playbook, AppError>;
    async fn get_playbook_by_id(&self, id: Uuid) -> Result<Playbook, AppError>;
    async fn list_playbooks(
        &self,
        tenant_id: Uuid,
        filters: &PlaybookFilters,
    ) -> Result<Vec<Playbook>, AppError>;
    async fn update_playbook(&self, id: Uuid, req: &UpdatePlaybook) -> Result<Playbook, AppError>;
    async fn fork_playbook(
        &self,
        tenant_id: Uuid,
        source: &Playbook,
        req: &UpdatePlaybook,
        created_by: Uuid,
    ) -> Result<Playbook, AppError>;
    async fn sync_playbook_with_parent(&self, id: Uuid) -> Result<Playbook, AppError>;
    async fn delete_playbook(&self, id: Uuid) -> Result<(), AppError>;

    // ── Step Templates ───────────────────────────────────────────────────────
    async fn create_step_template(
        &self,
        tenant_id: Uuid,
        req: &CreateStepTemplate,
        created_by: Uuid,
    ) -> Result<StepTemplate, AppError>;
    async fn get_step_template_by_id(&self, id: Uuid) -> Result<StepTemplate, AppError>;
    async fn list_step_templates(
        &self,
        tenant_id: Uuid,
        filters: &StepTemplateFilters,
    ) -> Result<Vec<StepTemplate>, AppError>;
    async fn update_step_template(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        req: &UpdateStepTemplate,
    ) -> Result<StepTemplate, AppError>;
    async fn fork_step_template(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        req: &UpdateStepTemplate,
        created_by: Uuid,
    ) -> Result<StepTemplate, AppError>;
    async fn delete_step_template(&self, id: Uuid, tenant_id: Uuid) -> Result<(), AppError>;

    // ── Events ────────────────────────────────────────────────────────────────
    async fn create_event(&self, project_id: Uuid, req: &CreateEvent) -> Result<Event, AppError>;
    async fn get_event_by_id(&self, id: Uuid) -> Result<Event, AppError>;
    async fn list_events(
        &self,
        project_id: Uuid,
        filters: &EventFilters,
    ) -> Result<Vec<Event>, AppError>;
    async fn list_recent_events(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Event>, AppError>;
    async fn count_events(&self, project_id: Uuid, filters: &EventFilters)
    -> Result<i64, AppError>;

    // ── Integrations ──────────────────────────────────────────────────────────
    async fn create_integration(
        &self,
        project_id: Uuid,
        req: &CreateIntegration,
    ) -> Result<Integration, AppError>;
    async fn get_integration(&self, id: Uuid) -> Result<Integration, AppError>;
    async fn list_integrations(
        &self,
        project_id: Uuid,
        filters: &IntegrationFilters,
    ) -> Result<Vec<Integration>, AppError>;
    async fn update_integration(
        &self,
        id: Uuid,
        req: &UpdateIntegration,
    ) -> Result<Integration, AppError>;
    async fn delete_integration(&self, id: Uuid) -> Result<(), AppError>;
    async fn grant_agent_access(
        &self,
        integration_id: Uuid,
        agent_id: Uuid,
        permissions: Vec<String>,
    ) -> Result<AgentIntegration, AppError>;
    async fn revoke_agent_access(
        &self,
        integration_id: Uuid,
        agent_id: Uuid,
    ) -> Result<(), AppError>;
    async fn list_agent_integrations(&self, agent_id: Uuid) -> Result<Vec<Integration>, AppError>;
    async fn list_integration_agents(
        &self,
        integration_id: Uuid,
    ) -> Result<Vec<AgentIntegration>, AppError>;

    // ── Roles ─────────────────────────────────────────────────────────────────
    async fn create_role(&self, tenant_id: Uuid, req: &CreateRole) -> Result<Role, AppError>;
    async fn get_role(&self, id: Uuid) -> Result<Role, AppError>;
    async fn list_roles(&self, tenant_id: Uuid) -> Result<Vec<Role>, AppError>;
    async fn update_role(&self, id: Uuid, req: &UpdateRole) -> Result<Role, AppError>;
    async fn delete_role(&self, id: Uuid) -> Result<(), AppError>;

    // ── Members ───────────────────────────────────────────────────────────────
    async fn create_membership(
        &self,
        tenant_id: Uuid,
        req: &CreateMembership,
    ) -> Result<Membership, AppError>;
    async fn get_membership(&self, id: Uuid) -> Result<Membership, AppError>;
    async fn list_members(&self, tenant_id: Uuid) -> Result<Vec<Membership>, AppError>;
    /// Return all distinct agent IDs with an active membership in the given tenant.
    async fn list_tenant_agent_ids(&self, tenant_id: Uuid) -> Result<Vec<Uuid>, AppError>;
    async fn list_agent_memberships(
        &self,
        agent_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<Membership>, AppError>;
    async fn get_active_membership_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Result<Option<Membership>, AppError>;
    async fn update_membership(
        &self,
        id: Uuid,
        req: &UpdateMembership,
    ) -> Result<Membership, AppError>;
    async fn remove_membership(&self, id: Uuid) -> Result<(), AppError>;

    // ── Project Hierarchy ─────────────────────────────────────────────────────
    async fn get_project_children(&self, parent_id: Uuid) -> Result<Vec<Project>, AppError>;
    async fn get_project_tree(&self, root_id: Uuid) -> Result<Vec<Project>, AppError>;

    // ── Delegation ────────────────────────────────────────────────────────────
    async fn delegate_task(
        &self,
        task_id: Uuid,
        delegated_by: Uuid,
        to_agent_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<Task, AppError>;

    // ── Auth ──────────────────────────────────────────────────────────────────
    async fn check_authority(
        &self,
        agent_id: Uuid,
        _project_id: Uuid,
        required_authority: &str,
    ) -> Result<bool, AppError>;
    async fn check_membership_for_agent(
        &self,
        agent_id: Uuid,
        _project_id: Uuid,
    ) -> Result<bool, AppError>;
    /// Returns true if the agent has `manage` authority on any project in the tenant.
    async fn check_tenant_manage_authority(
        &self,
        agent_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<bool, AppError>;

    // ── Audit Log ─────────────────────────────────────────────────────────────
    #[allow(clippy::too_many_arguments)]
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
    ) -> Result<AuditEntry, AppError>;
    async fn list_audit_log(
        &self,
        project_id: Uuid,
        filters: &AuditFilters,
    ) -> Result<Vec<AuditEntry>, AppError>;
    async fn get_entity_history(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditEntry>, AppError>;
    async fn count_audit_log(
        &self,
        project_id: Uuid,
        filters: &AuditFilters,
    ) -> Result<i64, AppError>;

    // ── Agent Context ─────────────────────────────────────────────────────────
    async fn get_agent_context(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<AgentContext>, AppError>;

    // ── Count helpers ─────────────────────────────────────────────────────────
    async fn count_tasks(&self, project_id: Uuid, filters: &TaskFilters) -> Result<i64, AppError>;

    // ── Webhooks ──────────────────────────────────────────────────────────────
    async fn create_webhook(
        &self,
        project_id: Uuid,
        req: &CreateWebhook,
    ) -> Result<Webhook, AppError>;
    async fn get_webhook(&self, id: Uuid) -> Result<Webhook, AppError>;
    async fn list_webhooks(&self, project_id: Uuid) -> Result<Vec<Webhook>, AppError>;
    async fn update_webhook(&self, id: Uuid, req: &UpdateWebhook) -> Result<Webhook, AppError>;
    async fn delete_webhook(&self, id: Uuid) -> Result<(), AppError>;
    async fn list_webhook_deliveries(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDelivery>, AppError>;
    async fn list_webhook_dead_letters(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDeadLetter>, AppError>;

    // ── Metrics ───────────────────────────────────────────────────────────────
    async fn get_project_metrics(
        &self,
        project_id: Uuid,
        days: i32,
    ) -> Result<ProjectMetrics, AppError>;

    // ── Search ────────────────────────────────────────────────────────────────
    async fn search(
        &self,
        project_id: Uuid,
        query: &str,
        entity_types: Option<&[&str]>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<SearchResult>, i64), AppError>;

    // ── File Locks ────────────────────────────────────────────────────────────
    async fn list_file_locks(&self, project_id: Uuid) -> Result<Vec<FileLock>, AppError>;
    async fn acquire_file_locks(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        paths: &[String],
        agent_id: Uuid,
    ) -> Result<Vec<FileLock>, AppError>;
    async fn release_file_locks(&self, project_id: Uuid, task_id: Uuid) -> Result<u64, AppError>;
    async fn release_file_locks_for_task(&self, task_id: Uuid) -> Result<u64, AppError>;

    // ── Verifications ─────────────────────────────────────────────────────────
    async fn create_verification(
        &self,
        project_id: Uuid,
        req: &CreateVerification,
        agent_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> Result<Verification, AppError>;
    async fn list_verifications(
        &self,
        project_id: Uuid,
        filters: &VerificationFilters,
    ) -> Result<Vec<Verification>, AppError>;
    async fn count_verifications(
        &self,
        project_id: Uuid,
        filters: &VerificationFilters,
    ) -> Result<i64, AppError>;
    async fn get_verification_by_id(&self, id: Uuid) -> Result<Verification, AppError>;
    async fn update_verification(
        &self,
        id: Uuid,
        req: &UpdateVerification,
    ) -> Result<Verification, AppError>;

    // ── Changed Files ─────────────────────────────────────────────────────────
    async fn create_changed_files(
        &self,
        task_id: Uuid,
        req: &CreateChangedFiles,
    ) -> Result<Vec<ChangedFileSummary>, AppError>;
    async fn list_changed_files(&self, task_id: Uuid) -> Result<Vec<ChangedFileSummary>, AppError>;
    async fn get_changed_file_by_id(&self, id: Uuid) -> Result<ChangedFile, AppError>;

    // ── Stale task detector (internal use) ───────────────────────────────────
    async fn query_stale_tasks(&self, default_timeout: i64) -> anyhow::Result<Vec<StaleTaskInfo>>;
    async fn release_stale_task_conditional(&self, task_id: Uuid) -> anyhow::Result<bool>;
    async fn mark_agent_offline(&self, agent_id: Uuid) -> anyhow::Result<()>;
    async fn get_project_timeout(&self, project_id: Uuid) -> i64;

    // ── Stale agent detector (internal use) ──────────────────────────────────
    /// Mark idle/working agents as offline when they haven't sent a heartbeat
    /// within `threshold_seconds`. Returns the IDs of agents marked offline.
    async fn mark_inactive_agents_offline(
        &self,
        threshold_seconds: i64,
    ) -> anyhow::Result<Vec<Uuid>>;
    /// Revoke all agents whose last heartbeat (or creation time if never seen)
    /// is older than `threshold_days`. Returns the IDs of agents that were revoked.
    async fn revoke_stale_agents(&self, threshold_days: i64) -> anyhow::Result<Vec<Uuid>>;

    // ── Auth User ────────────────────────────────────────────────────────────
    async fn resolve_or_create_user(&self, auth_user_id: &str) -> Result<Uuid, AppError>;

    /// Ensure an auth_user row exists for a specific user_id (used by dev auth
    /// bypasses where the user_id is known but may not have a DB record yet).
    async fn ensure_dev_user(&self, user_id: Uuid) -> Result<(), AppError>;

    // ── Webhook dispatch (internal use) ──────────────────────────────────────
    async fn list_webhooks_enabled(&self, project_id: Uuid) -> anyhow::Result<Vec<Webhook>>;
    async fn record_webhook_delivery(
        &self,
        webhook_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        status: Option<i32>,
        response_body: Option<&str>,
        success: bool,
        attempt: i32,
    );
    async fn record_webhook_dead_letter(
        &self,
        webhook_id: Uuid,
        event_type: &str,
        payload: &serde_json::Value,
        last_status: Option<i32>,
        last_body: Option<&str>,
        attempts: i32,
    );

    // ── Packages ─────────────────────────────────────────────────────────────
    /// Fetch the package assigned to a project. Returns None when the project
    /// has no package_id (e.g. before migration 023 runs).
    // ── Tenants ───────────────────────────────────────────────────────────────
    async fn create_tenant(&self, req: &CreateTenant) -> Result<Tenant, AppError>;
    async fn get_tenant_by_id(&self, id: Uuid) -> Result<Tenant, AppError>;
    async fn get_tenant_by_slug(&self, slug: &str) -> Result<Tenant, AppError>;
    async fn list_tenants(&self, filters: &TenantFilters) -> Result<Vec<Tenant>, AppError>;
    async fn update_tenant(&self, id: Uuid, req: &UpdateTenant) -> Result<Tenant, AppError>;
    async fn delete_tenant(&self, id: Uuid) -> Result<(), AppError>;

    // ── Tenant Members ───────────────────────────────────────────────────────
    async fn add_tenant_member(
        &self,
        tenant_id: Uuid,
        req: &AddTenantMember,
    ) -> Result<TenantMember, AppError>;
    async fn list_tenant_members(&self, tenant_id: Uuid) -> Result<Vec<TenantMember>, AppError>;
    async fn update_tenant_member(
        &self,
        member_id: Uuid,
        req: &UpdateTenantMember,
    ) -> Result<TenantMember, AppError>;
    async fn remove_tenant_member(&self, member_id: Uuid) -> Result<(), AppError>;
    async fn get_tenant_member_for_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TenantMember>, AppError>;
    async fn get_tenant_for_user(&self, user_id: Uuid) -> Result<Option<Tenant>, AppError>;

    // ── Wrapped Keys ─────────────────────────────────────────────────────────
    async fn create_wrapped_key(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        req: &CreateWrappedKey,
    ) -> Result<WrappedKey, AppError>;
    async fn list_wrapped_keys(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<WrappedKey>, AppError>;
    async fn delete_wrapped_key(&self, key_id: Uuid) -> Result<(), AppError>;

    // ── Packages ─────────────────────────────────────────────────────────────
    async fn get_package_for_project(&self, project_id: Uuid) -> Result<Option<Package>, AppError>;
    async fn list_packages(&self) -> Result<Vec<Package>, AppError>;
    async fn get_package_by_id(&self, id: Uuid) -> Result<Package, AppError>;
    async fn get_package_by_slug(&self, slug: &str) -> Result<Package, AppError>;
    async fn create_package(&self, req: &CreatePackage) -> Result<Package, AppError>;
    async fn update_package(&self, id: Uuid, req: &UpdatePackage) -> Result<Package, AppError>;
    async fn delete_package(&self, id: Uuid) -> Result<(), AppError>;

    // ── Reports ──────────────────────────────────────────────────────────────
    async fn create_report(
        &self,
        project_id: Uuid,
        req: &CreateReport,
        created_by: Uuid,
    ) -> Result<Report, AppError>;
    async fn get_report_by_id(&self, id: Uuid) -> Result<Report, AppError>;
    async fn list_reports(
        &self,
        project_id: Uuid,
        filters: &ReportFilters,
    ) -> Result<Vec<Report>, AppError>;
    async fn count_reports(
        &self,
        project_id: Uuid,
        filters: &ReportFilters,
    ) -> Result<i64, AppError>;
    async fn update_report(&self, id: Uuid, req: &UpdateReport) -> Result<Report, AppError>;
    async fn get_report_by_task_id(&self, task_id: Uuid) -> Result<Option<Report>, AppError>;
    async fn delete_report(&self, id: Uuid) -> Result<(), AppError>;

    // ── Task Logs ─────────────────────────────────────────────────────────────
    async fn create_task_log(
        &self,
        project_id: Uuid,
        agent_id: Option<Uuid>,
        req: &CreateTaskLog,
    ) -> Result<TaskLog, AppError>;
    async fn list_task_logs(
        &self,
        project_id: Uuid,
        filters: &TaskLogFilters,
    ) -> Result<Vec<TaskLogSummary>, AppError>;
    async fn count_task_logs(
        &self,
        project_id: Uuid,
        filters: &TaskLogFilters,
    ) -> Result<i64, AppError>;
    async fn get_task_log_by_id(&self, id: Uuid) -> Result<TaskLog, AppError>;
}
