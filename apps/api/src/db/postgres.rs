//! Postgres implementation of [`DiraigentDb`] — delegates to `repository::*`.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use super::DiraigentDb;
use crate::error::AppError;
use crate::models::*;
use crate::repository;

pub struct PostgresDb(pub PgPool);

impl PostgresDb {
    pub fn pool(&self) -> &PgPool {
        &self.0
    }
}

#[async_trait]
impl DiraigentDb for PostgresDb {
    async fn health_check(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.0).await.is_ok()
    }

    // Projects
    async fn create_project(
        &self,
        req: &CreateProject,
        owner_id: Uuid,
    ) -> Result<Project, AppError> {
        repository::create_project(&self.0, req, owner_id).await
    }
    async fn get_project_by_id(&self, id: Uuid) -> Result<Project, AppError> {
        repository::get_project_by_id(&self.0, id).await
    }
    async fn get_project_by_slug(&self, slug: &str) -> Result<Project, AppError> {
        repository::get_project_by_slug(&self.0, slug).await
    }
    async fn list_projects(&self, p: &Pagination) -> Result<Vec<Project>, AppError> {
        repository::list_projects(&self.0, p).await
    }
    async fn list_projects_for_tenant(
        &self,
        tenant_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<Project>, AppError> {
        repository::list_projects_for_tenant(&self.0, tenant_id, p).await
    }
    async fn update_project(&self, id: Uuid, req: &UpdateProject) -> Result<Project, AppError> {
        repository::update_project(&self.0, id, req).await
    }
    async fn delete_project(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_project(&self.0, id).await
    }

    // Tasks
    async fn create_task(
        &self,
        project_id: Uuid,
        req: &CreateTask,
        created_by: Uuid,
    ) -> Result<Task, AppError> {
        repository::create_task(&self.0, project_id, req, created_by).await
    }
    async fn get_task_by_id(&self, task_id: Uuid) -> Result<Task, AppError> {
        repository::get_task_by_id(&self.0, task_id).await
    }
    async fn list_tasks(
        &self,
        project_id: Uuid,
        filters: &TaskFilters,
    ) -> Result<Vec<Task>, AppError> {
        repository::list_tasks(&self.0, project_id, filters).await
    }
    async fn list_ready_tasks(
        &self,
        project_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError> {
        repository::list_ready_tasks(&self.0, project_id, limit, offset).await
    }
    async fn update_task(&self, task_id: Uuid, req: &UpdateTask) -> Result<Task, AppError> {
        repository::update_task(&self.0, task_id, req).await
    }
    async fn transition_task(
        &self,
        task_id: Uuid,
        target_state: &str,
        playbook_step: Option<i32>,
    ) -> Result<Task, AppError> {
        repository::transition_task(&self.0, task_id, target_state, playbook_step).await
    }
    async fn claim_task(&self, task_id: Uuid, agent_id: Uuid) -> Result<Task, AppError> {
        repository::claim_task(&self.0, task_id, agent_id).await
    }
    async fn resolve_claim_step_name(&self, task: &Task) -> Result<String, AppError> {
        repository::resolve_claim_step_name(&self.0, task).await
    }
    async fn release_task(&self, task_id: Uuid) -> Result<Task, AppError> {
        repository::release_task(&self.0, task_id).await
    }
    async fn delete_task(&self, task_id: Uuid) -> Result<(), AppError> {
        repository::delete_task(&self.0, task_id).await
    }
    async fn list_subtasks(
        &self,
        parent_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError> {
        repository::list_subtasks(&self.0, parent_id, limit, offset).await
    }
    async fn count_subtasks(&self, parent_id: Uuid) -> Result<i64, AppError> {
        repository::count_subtasks(&self.0, parent_id).await
    }
    async fn update_task_cost(
        &self,
        task_id: Uuid,
        input_tokens: i64,
        output_tokens: i64,
        cost_usd: f64,
    ) -> Result<Task, AppError> {
        repository::update_task_cost(&self.0, task_id, input_tokens, output_tokens, cost_usd).await
    }

    // Dependencies
    async fn add_dependency(
        &self,
        task_id: Uuid,
        depends_on: Uuid,
    ) -> Result<TaskDependency, AppError> {
        repository::add_dependency(&self.0, task_id, depends_on).await
    }
    async fn remove_dependency(&self, task_id: Uuid, depends_on: Uuid) -> Result<(), AppError> {
        repository::remove_dependency(&self.0, task_id, depends_on).await
    }
    async fn list_dependencies(&self, task_id: Uuid) -> Result<TaskDependencies, AppError> {
        repository::list_dependencies(&self.0, task_id).await
    }
    async fn list_blocked_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        repository::list_blocked_task_ids(&self.0, project_id).await
    }
    async fn list_flagged_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        repository::list_flagged_task_ids(&self.0, project_id).await
    }
    async fn list_goal_linked_task_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        repository::list_goal_linked_task_ids(&self.0, project_id).await
    }
    async fn list_tasks_with_blocker_updates(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Task>, AppError> {
        repository::list_tasks_with_blocker_updates(&self.0, project_id).await
    }
    async fn list_task_children(&self, parent_id: Uuid) -> Result<Vec<Task>, AppError> {
        repository::list_task_children(&self.0, parent_id).await
    }

    // Task Updates
    async fn create_task_update(
        &self,
        task_id: Uuid,
        req: &CreateTaskUpdate,
        user_id: Option<Uuid>,
    ) -> Result<TaskUpdate, AppError> {
        repository::create_task_update(&self.0, task_id, req, user_id).await
    }
    async fn list_task_updates(
        &self,
        task_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<TaskUpdate>, AppError> {
        repository::list_task_updates(&self.0, task_id, p).await
    }

    // Task Comments
    async fn create_task_comment(
        &self,
        task_id: Uuid,
        req: &CreateTaskComment,
        user_id: Option<Uuid>,
    ) -> Result<TaskComment, AppError> {
        repository::create_task_comment(&self.0, task_id, req, user_id).await
    }
    async fn list_task_comments(
        &self,
        task_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<TaskComment>, AppError> {
        repository::list_task_comments(&self.0, task_id, p).await
    }

    // Agents
    async fn register_agent(
        &self,
        req: &CreateAgent,
        owner_id: Uuid,
    ) -> Result<(Agent, String), AppError> {
        repository::register_agent(&self.0, req, owner_id).await
    }
    async fn authenticate_agent_key(
        &self,
        key_hash: &str,
    ) -> Result<Option<(Uuid, Uuid)>, AppError> {
        repository::authenticate_agent_key(&self.0, key_hash).await
    }
    async fn get_agent_by_id(&self, id: Uuid) -> Result<Agent, AppError> {
        repository::get_agent_by_id(&self.0, id).await
    }
    async fn list_agents(&self, p: &Pagination) -> Result<Vec<Agent>, AppError> {
        repository::list_agents(&self.0, p).await
    }
    async fn update_agent(&self, id: Uuid, req: &UpdateAgent) -> Result<Agent, AppError> {
        repository::update_agent(&self.0, id, req).await
    }
    async fn agent_heartbeat(&self, id: Uuid, status: Option<&str>) -> Result<Agent, AppError> {
        repository::agent_heartbeat(&self.0, id, status).await
    }
    async fn list_agent_tasks(
        &self,
        agent_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<Task>, AppError> {
        repository::list_agent_tasks(&self.0, agent_id, p).await
    }
    async fn verify_agent_owner(&self, agent_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        repository::verify_agent_owner(&self.0, agent_id, user_id).await
    }

    // Goals
    async fn create_goal(
        &self,
        project_id: Uuid,
        req: &CreateGoal,
        created_by: Uuid,
    ) -> Result<Goal, AppError> {
        repository::create_goal(&self.0, project_id, req, created_by).await
    }
    async fn get_goal_by_id(&self, id: Uuid) -> Result<Goal, AppError> {
        repository::get_goal_by_id(&self.0, id).await
    }
    async fn list_goals(
        &self,
        project_id: Uuid,
        filters: &GoalFilters,
    ) -> Result<Vec<Goal>, AppError> {
        repository::list_goals(&self.0, project_id, filters).await
    }
    async fn activate_goal(&self, goal_id: Uuid) -> Result<Goal, AppError> {
        repository::activate_goal(&self.0, goal_id).await
    }
    async fn update_goal(&self, id: Uuid, req: &UpdateGoal) -> Result<Goal, AppError> {
        repository::update_goal(&self.0, id, req).await
    }
    async fn delete_goal(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_goal(&self.0, id).await
    }
    async fn link_task_goal(&self, goal_id: Uuid, task_id: Uuid) -> Result<TaskGoal, AppError> {
        repository::link_task_goal(&self.0, goal_id, task_id).await
    }
    async fn unlink_task_goal(&self, goal_id: Uuid, task_id: Uuid) -> Result<(), AppError> {
        repository::unlink_task_goal(&self.0, goal_id, task_id).await
    }
    async fn get_goal_progress(&self, goal_id: Uuid) -> Result<GoalProgress, AppError> {
        repository::get_goal_progress(&self.0, goal_id).await
    }
    async fn list_goal_tasks(
        &self,
        goal_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Task>, AppError> {
        repository::list_goal_tasks(&self.0, goal_id, limit, offset).await
    }
    async fn count_goal_tasks(&self, goal_id: Uuid) -> Result<i64, AppError> {
        repository::count_goal_tasks(&self.0, goal_id).await
    }
    async fn bulk_link_tasks(&self, goal_id: Uuid, task_ids: &[Uuid]) -> Result<i64, AppError> {
        repository::bulk_link_tasks(&self.0, goal_id, task_ids).await
    }
    async fn get_goal_stats(&self, goal_id: Uuid) -> Result<GoalStats, AppError> {
        repository::get_goal_stats(&self.0, goal_id).await
    }
    async fn compute_auto_status(&self, goal_id: Uuid) -> Result<Option<String>, AppError> {
        repository::compute_auto_status(&self.0, goal_id).await
    }
    async fn list_auto_status_goal_ids_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        repository::list_auto_status_goal_ids_for_task(&self.0, task_id).await
    }
    async fn reorder_goals(
        &self,
        project_id: Uuid,
        goal_ids: &[Uuid],
    ) -> Result<Vec<Goal>, AppError> {
        repository::reorder_goals(&self.0, project_id, goal_ids).await
    }
    async fn get_goal_ids_for_task(&self, task_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        repository::get_goal_ids_for_task(&self.0, task_id).await
    }
    async fn get_agent_inherited_goal_ids(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
        exclude_task_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        repository::get_agent_inherited_goal_ids(&self.0, agent_id, project_id, exclude_task_id)
            .await
    }
    async fn list_goals_for_task(&self, task_id: Uuid) -> Result<Vec<Goal>, AppError> {
        repository::list_goals_for_task(&self.0, task_id).await
    }

    // Goal Comments
    async fn create_goal_comment(
        &self,
        goal_id: Uuid,
        req: &CreateGoalComment,
        user_id: Option<Uuid>,
    ) -> Result<GoalComment, AppError> {
        repository::create_goal_comment(&self.0, goal_id, req, user_id).await
    }
    async fn list_goal_comments(
        &self,
        goal_id: Uuid,
        p: &Pagination,
    ) -> Result<Vec<GoalComment>, AppError> {
        repository::list_goal_comments(&self.0, goal_id, p).await
    }

    // Knowledge
    async fn create_knowledge(
        &self,
        project_id: Uuid,
        req: &CreateKnowledge,
        created_by: Uuid,
    ) -> Result<Knowledge, AppError> {
        repository::create_knowledge(&self.0, project_id, req, created_by).await
    }
    async fn get_knowledge_by_id(&self, id: Uuid) -> Result<Knowledge, AppError> {
        repository::get_knowledge_by_id(&self.0, id).await
    }
    async fn list_knowledge(
        &self,
        project_id: Uuid,
        filters: &KnowledgeFilters,
    ) -> Result<Vec<Knowledge>, AppError> {
        repository::list_knowledge(&self.0, project_id, filters).await
    }
    async fn update_knowledge(
        &self,
        id: Uuid,
        req: &UpdateKnowledge,
    ) -> Result<Knowledge, AppError> {
        repository::update_knowledge(&self.0, id, req).await
    }
    async fn delete_knowledge(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_knowledge(&self.0, id).await
    }
    async fn count_knowledge(
        &self,
        project_id: Uuid,
        filters: &KnowledgeFilters,
    ) -> Result<i64, AppError> {
        repository::count_knowledge(&self.0, project_id, filters).await
    }
    async fn update_knowledge_embedding(
        &self,
        id: Uuid,
        embedding: &[f64],
    ) -> Result<(), AppError> {
        repository::update_knowledge_embedding(&self.0, id, embedding).await
    }
    async fn list_knowledge_with_embeddings(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Knowledge>, AppError> {
        repository::list_knowledge_with_embeddings(&self.0, project_id).await
    }

    // Decisions
    async fn create_decision(
        &self,
        project_id: Uuid,
        req: &CreateDecision,
        created_by: Uuid,
    ) -> Result<Decision, AppError> {
        repository::create_decision(&self.0, project_id, req, created_by).await
    }
    async fn get_decision_by_id(&self, id: Uuid) -> Result<Decision, AppError> {
        repository::get_decision_by_id(&self.0, id).await
    }
    async fn list_decisions(
        &self,
        project_id: Uuid,
        filters: &DecisionFilters,
    ) -> Result<Vec<Decision>, AppError> {
        repository::list_decisions(&self.0, project_id, filters).await
    }
    async fn update_decision(&self, id: Uuid, req: &UpdateDecision) -> Result<Decision, AppError> {
        repository::update_decision(&self.0, id, req).await
    }
    async fn delete_decision(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_decision(&self.0, id).await
    }
    async fn count_decisions(
        &self,
        project_id: Uuid,
        filters: &DecisionFilters,
    ) -> Result<i64, AppError> {
        repository::count_decisions(&self.0, project_id, filters).await
    }
    async fn list_tasks_by_decision(
        &self,
        decision_id: Uuid,
    ) -> Result<Vec<TaskSummaryForDecision>, AppError> {
        repository::list_tasks_by_decision(&self.0, decision_id).await
    }

    // Observations
    async fn create_observation(
        &self,
        project_id: Uuid,
        req: &CreateObservation,
    ) -> Result<Observation, AppError> {
        repository::create_observation(&self.0, project_id, req).await
    }
    async fn get_observation_by_id(&self, id: Uuid) -> Result<Observation, AppError> {
        repository::get_observation_by_id(&self.0, id).await
    }
    async fn list_observations(
        &self,
        project_id: Uuid,
        filters: &ObservationFilters,
    ) -> Result<Vec<Observation>, AppError> {
        repository::list_observations(&self.0, project_id, filters).await
    }
    async fn update_observation(
        &self,
        id: Uuid,
        req: &UpdateObservation,
    ) -> Result<Observation, AppError> {
        repository::update_observation(&self.0, id, req).await
    }
    async fn dismiss_observation(&self, id: Uuid) -> Result<Observation, AppError> {
        repository::dismiss_observation(&self.0, id).await
    }
    async fn promote_observation(
        &self,
        obs_id: Uuid,
        req: &PromoteObservation,
        created_by: Uuid,
    ) -> Result<(Observation, Task), AppError> {
        repository::promote_observation(&self.0, obs_id, req, created_by).await
    }
    async fn count_observations(
        &self,
        project_id: Uuid,
        filters: &ObservationFilters,
    ) -> Result<i64, AppError> {
        repository::count_observations(&self.0, project_id, filters).await
    }
    async fn delete_observation(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_observation(&self.0, id).await
    }
    async fn cleanup_observations(
        &self,
        project_id: Uuid,
    ) -> Result<CleanupObservationsResult, AppError> {
        repository::cleanup_observations(&self.0, project_id).await
    }
    async fn delete_old_observations_all_projects(
        &self,
        default_retention_days: i32,
    ) -> Result<u64, AppError> {
        repository::delete_old_observations_all_projects(&self.0, default_retention_days).await
    }

    // Goal task reordering
    async fn reorder_goal_tasks(
        &self,
        goal_id: Uuid,
        task_ids: &[Uuid],
    ) -> Result<Vec<Task>, AppError> {
        repository::reorder_goal_tasks(&self.0, goal_id, task_ids).await
    }

    // Playbooks
    async fn create_playbook(
        &self,
        tenant_id: Uuid,
        req: &CreatePlaybook,
        created_by: Uuid,
    ) -> Result<Playbook, AppError> {
        repository::create_playbook(&self.0, tenant_id, req, created_by).await
    }
    async fn get_playbook_by_id(&self, id: Uuid) -> Result<Playbook, AppError> {
        repository::get_playbook_by_id(&self.0, id).await
    }
    async fn list_playbooks(
        &self,
        tenant_id: Uuid,
        filters: &PlaybookFilters,
    ) -> Result<Vec<Playbook>, AppError> {
        repository::list_playbooks(&self.0, tenant_id, filters).await
    }
    async fn update_playbook(&self, id: Uuid, req: &UpdatePlaybook) -> Result<Playbook, AppError> {
        repository::update_playbook(&self.0, id, req).await
    }
    async fn fork_playbook(
        &self,
        tenant_id: Uuid,
        source: &Playbook,
        req: &UpdatePlaybook,
        created_by: Uuid,
    ) -> Result<Playbook, AppError> {
        repository::fork_playbook(&self.0, tenant_id, source, req, created_by).await
    }
    async fn sync_playbook_with_parent(&self, id: Uuid) -> Result<Playbook, AppError> {
        repository::sync_playbook_with_parent(&self.0, id).await
    }
    async fn delete_playbook(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_playbook(&self.0, id).await
    }

    // Step Templates
    async fn create_step_template(
        &self,
        tenant_id: Uuid,
        req: &CreateStepTemplate,
        created_by: Uuid,
    ) -> Result<StepTemplate, AppError> {
        repository::create_step_template(&self.0, tenant_id, req, created_by).await
    }
    async fn get_step_template_by_id(&self, id: Uuid) -> Result<StepTemplate, AppError> {
        repository::get_step_template_by_id(&self.0, id).await
    }
    async fn list_step_templates(
        &self,
        tenant_id: Uuid,
        filters: &StepTemplateFilters,
    ) -> Result<Vec<StepTemplate>, AppError> {
        repository::list_step_templates(&self.0, tenant_id, filters).await
    }
    async fn update_step_template(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        req: &UpdateStepTemplate,
    ) -> Result<StepTemplate, AppError> {
        repository::update_step_template(&self.0, id, tenant_id, req).await
    }
    async fn fork_step_template(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        req: &UpdateStepTemplate,
        created_by: Uuid,
    ) -> Result<StepTemplate, AppError> {
        repository::fork_step_template(&self.0, id, tenant_id, req, created_by).await
    }
    async fn delete_step_template(&self, id: Uuid, tenant_id: Uuid) -> Result<(), AppError> {
        repository::delete_step_template(&self.0, id, tenant_id).await
    }

    // Events
    async fn create_event(&self, project_id: Uuid, req: &CreateEvent) -> Result<Event, AppError> {
        repository::create_event(&self.0, project_id, req).await
    }
    async fn get_event_by_id(&self, id: Uuid) -> Result<Event, AppError> {
        repository::get_event_by_id(&self.0, id).await
    }
    async fn list_events(
        &self,
        project_id: Uuid,
        filters: &EventFilters,
    ) -> Result<Vec<Event>, AppError> {
        repository::list_events(&self.0, project_id, filters).await
    }
    async fn list_recent_events(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Event>, AppError> {
        repository::list_recent_events(&self.0, project_id, limit).await
    }
    async fn count_events(
        &self,
        project_id: Uuid,
        filters: &EventFilters,
    ) -> Result<i64, AppError> {
        repository::count_events(&self.0, project_id, filters).await
    }

    // Integrations
    async fn create_integration(
        &self,
        project_id: Uuid,
        req: &CreateIntegration,
    ) -> Result<Integration, AppError> {
        repository::create_integration(&self.0, project_id, req).await
    }
    async fn get_integration(&self, id: Uuid) -> Result<Integration, AppError> {
        repository::get_integration(&self.0, id).await
    }
    async fn list_integrations(
        &self,
        project_id: Uuid,
        filters: &IntegrationFilters,
    ) -> Result<Vec<Integration>, AppError> {
        repository::list_integrations(&self.0, project_id, filters).await
    }
    async fn update_integration(
        &self,
        id: Uuid,
        req: &UpdateIntegration,
    ) -> Result<Integration, AppError> {
        repository::update_integration(&self.0, id, req).await
    }
    async fn delete_integration(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_integration(&self.0, id).await
    }
    async fn grant_agent_access(
        &self,
        integration_id: Uuid,
        agent_id: Uuid,
        permissions: Vec<String>,
    ) -> Result<AgentIntegration, AppError> {
        repository::grant_agent_access(&self.0, integration_id, agent_id, &permissions, None).await
    }
    async fn revoke_agent_access(
        &self,
        integration_id: Uuid,
        agent_id: Uuid,
    ) -> Result<(), AppError> {
        repository::revoke_agent_access(&self.0, integration_id, agent_id).await
    }
    async fn list_agent_integrations(&self, agent_id: Uuid) -> Result<Vec<Integration>, AppError> {
        repository::list_agent_integrations(&self.0, agent_id).await
    }
    async fn list_integration_agents(
        &self,
        integration_id: Uuid,
    ) -> Result<Vec<AgentIntegration>, AppError> {
        repository::list_integration_agents(&self.0, integration_id).await
    }

    // Roles
    async fn create_role(&self, tenant_id: Uuid, req: &CreateRole) -> Result<Role, AppError> {
        repository::create_role(&self.0, tenant_id, req).await
    }
    async fn get_role(&self, id: Uuid) -> Result<Role, AppError> {
        repository::get_role(&self.0, id).await
    }
    async fn list_roles(&self, tenant_id: Uuid) -> Result<Vec<Role>, AppError> {
        repository::list_roles(&self.0, tenant_id).await
    }
    async fn update_role(&self, id: Uuid, req: &UpdateRole) -> Result<Role, AppError> {
        repository::update_role(&self.0, id, req).await
    }
    async fn delete_role(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_role(&self.0, id).await
    }

    // Members
    async fn create_membership(
        &self,
        tenant_id: Uuid,
        req: &CreateMembership,
    ) -> Result<Membership, AppError> {
        repository::create_membership(&self.0, tenant_id, req).await
    }
    async fn get_membership(&self, id: Uuid) -> Result<Membership, AppError> {
        repository::get_membership(&self.0, id).await
    }
    async fn list_members(&self, tenant_id: Uuid) -> Result<Vec<Membership>, AppError> {
        repository::list_members(&self.0, tenant_id).await
    }
    async fn list_tenant_agent_ids(&self, tenant_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        repository::list_tenant_agent_ids(&self.0, tenant_id).await
    }
    async fn list_agent_memberships(
        &self,
        agent_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<Membership>, AppError> {
        repository::list_agent_memberships(&self.0, agent_id, tenant_id).await
    }
    async fn get_active_membership_by_agent(
        &self,
        agent_id: Uuid,
    ) -> Result<Option<Membership>, AppError> {
        let m = sqlx::query_as::<_, Membership>(
            "SELECT * FROM diraigent.membership WHERE agent_id = $1 AND status = 'active' LIMIT 1",
        )
        .bind(agent_id)
        .fetch_optional(&self.0)
        .await?;
        Ok(m)
    }
    async fn update_membership(
        &self,
        id: Uuid,
        req: &UpdateMembership,
    ) -> Result<Membership, AppError> {
        repository::update_membership(&self.0, id, req).await
    }
    async fn remove_membership(&self, id: Uuid) -> Result<(), AppError> {
        repository::remove_membership(&self.0, id).await
    }

    // Hierarchy
    async fn get_project_children(&self, parent_id: Uuid) -> Result<Vec<Project>, AppError> {
        repository::get_project_children(&self.0, parent_id).await
    }
    async fn get_project_tree(&self, root_id: Uuid) -> Result<Vec<Project>, AppError> {
        repository::get_project_tree(&self.0, root_id).await
    }

    // Delegation
    async fn delegate_task(
        &self,
        task_id: Uuid,
        delegated_by: Uuid,
        to_agent_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<Task, AppError> {
        repository::delegate_task(&self.0, task_id, delegated_by, to_agent_id, role_id).await
    }

    // Auth
    async fn check_authority(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
        required_authority: &str,
    ) -> Result<bool, AppError> {
        repository::check_authority(&self.0, agent_id, project_id, required_authority).await
    }
    async fn check_membership_for_agent(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, AppError> {
        repository::check_membership(&self.0, agent_id, project_id).await
    }
    async fn check_tenant_manage_authority(
        &self,
        agent_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<bool, AppError> {
        repository::check_tenant_manage_authority(&self.0, agent_id, tenant_id).await
    }

    // Audit
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
        repository::create_audit_entry(
            &self.0,
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
        repository::list_audit_log(&self.0, project_id, filters).await
    }
    async fn get_entity_history(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditEntry>, AppError> {
        repository::get_entity_history(&self.0, entity_type, entity_id, limit).await
    }
    async fn count_audit_log(
        &self,
        project_id: Uuid,
        filters: &AuditFilters,
    ) -> Result<i64, AppError> {
        repository::count_audit_log(&self.0, project_id, filters).await
    }

    // Agent Context
    async fn get_agent_context(
        &self,
        agent_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<AgentContext>, AppError> {
        repository::get_agent_context(&self.0, agent_id, project_id).await
    }

    // Counts
    async fn count_tasks(&self, project_id: Uuid, filters: &TaskFilters) -> Result<i64, AppError> {
        repository::count_tasks(&self.0, project_id, filters).await
    }

    // Webhooks
    async fn create_webhook(
        &self,
        project_id: Uuid,
        req: &CreateWebhook,
    ) -> Result<Webhook, AppError> {
        repository::create_webhook(&self.0, project_id, req).await
    }
    async fn get_webhook(&self, id: Uuid) -> Result<Webhook, AppError> {
        repository::get_webhook(&self.0, id).await
    }
    async fn list_webhooks(&self, project_id: Uuid) -> Result<Vec<Webhook>, AppError> {
        repository::list_webhooks(&self.0, project_id).await
    }
    async fn update_webhook(&self, id: Uuid, req: &UpdateWebhook) -> Result<Webhook, AppError> {
        repository::update_webhook(&self.0, id, req).await
    }
    async fn delete_webhook(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_webhook(&self.0, id).await
    }
    async fn list_webhook_deliveries(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDelivery>, AppError> {
        repository::list_webhook_deliveries(&self.0, webhook_id, limit).await
    }
    async fn list_webhook_dead_letters(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDeadLetter>, AppError> {
        repository::list_webhook_dead_letters(&self.0, webhook_id, limit).await
    }

    // Metrics
    async fn get_project_metrics(
        &self,
        project_id: Uuid,
        days: i32,
    ) -> Result<ProjectMetrics, AppError> {
        repository::get_project_metrics(&self.0, project_id, days).await
    }

    // Search
    async fn search(
        &self,
        project_id: Uuid,
        query: &str,
        entity_types: Option<&[&str]>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<SearchResult>, i64), AppError> {
        repository::search(&self.0, project_id, query, entity_types, limit, offset).await
    }

    // File Locks
    async fn list_file_locks(&self, project_id: Uuid) -> Result<Vec<FileLock>, AppError> {
        repository::list_file_locks(&self.0, project_id).await
    }
    async fn acquire_file_locks(
        &self,
        project_id: Uuid,
        task_id: Uuid,
        paths: &[String],
        agent_id: Uuid,
    ) -> Result<Vec<FileLock>, AppError> {
        repository::acquire_file_locks(&self.0, project_id, task_id, paths, agent_id).await
    }
    async fn release_file_locks(&self, project_id: Uuid, task_id: Uuid) -> Result<u64, AppError> {
        repository::release_file_locks(&self.0, project_id, task_id).await
    }
    async fn release_file_locks_for_task(&self, task_id: Uuid) -> Result<u64, AppError> {
        repository::release_file_locks_for_task(&self.0, task_id).await
    }

    // Verifications
    async fn create_verification(
        &self,
        project_id: Uuid,
        req: &CreateVerification,
        agent_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> Result<Verification, AppError> {
        repository::create_verification(&self.0, project_id, agent_id, user_id, req).await
    }
    async fn list_verifications(
        &self,
        project_id: Uuid,
        filters: &VerificationFilters,
    ) -> Result<Vec<Verification>, AppError> {
        repository::list_verifications(&self.0, project_id, filters).await
    }
    async fn count_verifications(
        &self,
        project_id: Uuid,
        filters: &VerificationFilters,
    ) -> Result<i64, AppError> {
        repository::count_verifications(&self.0, project_id, filters).await
    }
    async fn get_verification_by_id(&self, id: Uuid) -> Result<Verification, AppError> {
        repository::get_verification_by_id(&self.0, id).await
    }
    async fn update_verification(
        &self,
        id: Uuid,
        req: &UpdateVerification,
    ) -> Result<Verification, AppError> {
        repository::update_verification(&self.0, id, req).await
    }

    // Changed Files
    async fn create_changed_files(
        &self,
        task_id: Uuid,
        req: &CreateChangedFiles,
    ) -> Result<Vec<ChangedFileSummary>, AppError> {
        repository::create_changed_files(&self.0, task_id, req).await
    }
    async fn list_changed_files(&self, task_id: Uuid) -> Result<Vec<ChangedFileSummary>, AppError> {
        repository::list_changed_files(&self.0, task_id).await
    }
    async fn get_changed_file_by_id(&self, id: Uuid) -> Result<ChangedFile, AppError> {
        repository::get_changed_file_by_id(&self.0, id).await
    }

    // Stale detector support
    async fn query_stale_tasks(&self, default_timeout: i64) -> anyhow::Result<Vec<StaleTaskInfo>> {
        #[derive(sqlx::FromRow)]
        struct PgStaleTask {
            task_id: Uuid,
            task_title: String,
            task_state: String,
            project_id: Uuid,
            agent_id: Uuid,
            agent_name: String,
            agent_last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
            claimed_at: Option<chrono::DateTime<chrono::Utc>>,
            auto_release: bool,
        }
        let rows = sqlx::query_as::<_, PgStaleTask>(
            "SELECT
                t.id AS task_id,
                t.title AS task_title,
                t.state AS task_state,
                t.project_id,
                a.id AS agent_id,
                a.name AS agent_name,
                a.last_seen_at AS agent_last_seen_at,
                t.claimed_at,
                COALESCE((p.metadata->'auto_release_stale_tasks')::boolean, true) AS auto_release
             FROM diraigent.task t
             JOIN diraigent.agent a ON t.assigned_agent_id = a.id
             JOIN diraigent.project p ON t.project_id = p.id
             WHERE t.assigned_agent_id IS NOT NULL
               AND t.state NOT IN ('backlog', 'ready', 'done', 'cancelled')
               AND a.last_seen_at < now() - make_interval(secs => COALESCE((p.metadata->'heartbeat_timeout_seconds')::double precision, $1::double precision))",
        )
        .bind(default_timeout as f64)
        .fetch_all(&self.0)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| StaleTaskInfo {
                task_id: r.task_id,
                task_title: r.task_title,
                task_state: r.task_state,
                project_id: r.project_id,
                agent_id: r.agent_id,
                agent_name: r.agent_name,
                agent_last_seen_at: r.agent_last_seen_at,
                claimed_at: r.claimed_at,
                auto_release: r.auto_release,
            })
            .collect())
    }

    async fn release_stale_task_conditional(&self, task_id: Uuid) -> anyhow::Result<bool> {
        let row = sqlx::query(
            "UPDATE diraigent.task
             SET state = 'ready', assigned_agent_id = NULL, claimed_at = NULL
             WHERE id = $1 AND state NOT IN ('backlog', 'ready', 'done', 'cancelled')
             RETURNING id",
        )
        .bind(task_id)
        .fetch_optional(&self.0)
        .await?;
        Ok(row.is_some())
    }

    async fn mark_agent_offline(&self, agent_id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE diraigent.agent SET status = 'offline' WHERE id = $1 AND status != 'offline'",
        )
        .bind(agent_id)
        .execute(&self.0)
        .await?;
        Ok(())
    }

    async fn get_project_timeout(&self, project_id: Uuid) -> i64 {
        let result: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT metadata FROM diraigent.project WHERE id = $1")
                .bind(project_id)
                .fetch_optional(&self.0)
                .await
                .ok()
                .flatten();
        result
            .and_then(|(m,)| m.get("heartbeat_timeout_seconds")?.as_i64())
            .unwrap_or(600)
    }

    async fn mark_inactive_agents_offline(
        &self,
        threshold_seconds: i64,
    ) -> anyhow::Result<Vec<Uuid>> {
        // Mark agents as offline if their last heartbeat is older than the threshold.
        // Only targets agents that are currently idle or working (not already offline/revoked).
        // Agents that have never sent a heartbeat are not marked offline here —
        // they may be newly registered and waiting to start.
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "UPDATE diraigent.agent
             SET status = 'offline', updated_at = now()
             WHERE status IN ('idle', 'working')
               AND last_seen_at IS NOT NULL
               AND last_seen_at < now() - ($1 || ' seconds')::interval
             RETURNING id",
        )
        .bind(threshold_seconds)
        .fetch_all(&self.0)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn revoke_stale_agents(&self, threshold_days: i64) -> anyhow::Result<Vec<Uuid>> {
        // Revoke agents that have not sent a heartbeat within the threshold.
        // An agent that has never sent a heartbeat uses its creation time instead.
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "UPDATE diraigent.agent
             SET status = 'revoked', updated_at = now()
             WHERE status NOT IN ('revoked')
               AND COALESCE(last_seen_at, created_at) < now() - ($1 || ' days')::interval
             RETURNING id",
        )
        .bind(threshold_days)
        .fetch_all(&self.0)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    // Webhook dispatch support
    async fn list_webhooks_enabled(&self, project_id: Uuid) -> anyhow::Result<Vec<Webhook>> {
        let hooks = sqlx::query_as::<_, Webhook>(
            "SELECT * FROM diraigent.webhook WHERE project_id = $1 AND enabled = true",
        )
        .bind(project_id)
        .fetch_all(&self.0)
        .await?;
        Ok(hooks)
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
        let _ = sqlx::query(
            "INSERT INTO diraigent.webhook_delivery (webhook_id, event_type, payload, response_status, response_body, success, attempt_number)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(webhook_id)
        .bind(event_type)
        .bind(payload)
        .bind(status)
        .bind(response_body)
        .bind(success)
        .bind(attempt)
        .execute(&self.0)
        .await;
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
        let _ = sqlx::query(
            "INSERT INTO diraigent.webhook_dead_letter (webhook_id, event_type, payload, last_response_status, last_response_body, attempts)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(webhook_id)
        .bind(event_type)
        .bind(payload)
        .bind(last_status)
        .bind(last_body)
        .bind(attempts)
        .execute(&self.0)
        .await;
    }
    // ── Auth User ─────────────────────────────────────────────────────────────
    async fn resolve_or_create_user(&self, auth_user_id: &str) -> Result<Uuid, AppError> {
        let row: (Uuid,) = sqlx::query_as(
            "INSERT INTO diraigent.auth_user (auth_user_id)
             VALUES ($1)
             ON CONFLICT (auth_user_id) DO UPDATE SET auth_user_id = EXCLUDED.auth_user_id
             RETURNING user_id",
        )
        .bind(auth_user_id)
        .fetch_one(&self.0)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let user_id = row.0;

        // Best-effort: auto-join the default tenant so new users can immediately
        // access tenant-scoped endpoints.  This is not fatal — if the default
        // tenant does not exist yet (e.g. fresh schema before migration 029), the
        // TenantContext extractor will create a personal workspace for the user
        // on their first tenant-scoped request.
        if let Err(e) = sqlx::query(
            "INSERT INTO diraigent.tenant_member (tenant_id, user_id, role)
             VALUES ('00000000-0000-0000-0000-000000000001', $1, 'member')
             ON CONFLICT (tenant_id, user_id) DO NOTHING",
        )
        .bind(user_id)
        .execute(&self.0)
        .await
        {
            tracing::warn!(
                user_id = %user_id,
                error = %e,
                "auto-join default tenant failed (will create personal workspace on next request)"
            );
        }

        Ok(user_id)
    }

    async fn ensure_dev_user(&self, user_id: Uuid) -> Result<(), AppError> {
        // Create an auth_user row with the explicit user_id if one doesn't exist.
        // Uses the stringified user_id as auth_user_id so it's deterministic.
        // Both user_id (PK) and auth_user_id (UNIQUE) may conflict independently,
        // so we check existence first to avoid partial-conflict errors.
        sqlx::query(
            "INSERT INTO diraigent.auth_user (user_id, auth_user_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(user_id.to_string())
        .execute(&self.0)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    // ── Tenants ───────────────────────────────────────────────────────────────
    async fn create_tenant(&self, req: &CreateTenant) -> Result<Tenant, AppError> {
        repository::create_tenant(&self.0, req).await
    }
    async fn get_tenant_by_id(&self, id: Uuid) -> Result<Tenant, AppError> {
        repository::get_tenant_by_id(&self.0, id).await
    }
    async fn get_tenant_by_slug(&self, slug: &str) -> Result<Tenant, AppError> {
        repository::get_tenant_by_slug(&self.0, slug).await
    }
    async fn list_tenants(&self, filters: &TenantFilters) -> Result<Vec<Tenant>, AppError> {
        repository::list_tenants(&self.0, filters).await
    }
    async fn update_tenant(&self, id: Uuid, req: &UpdateTenant) -> Result<Tenant, AppError> {
        repository::update_tenant(&self.0, id, req).await
    }
    async fn delete_tenant(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_tenant(&self.0, id).await
    }

    // Tenant Members
    async fn add_tenant_member(
        &self,
        tenant_id: Uuid,
        req: &AddTenantMember,
    ) -> Result<TenantMember, AppError> {
        repository::add_tenant_member(&self.0, tenant_id, req).await
    }
    async fn list_tenant_members(&self, tenant_id: Uuid) -> Result<Vec<TenantMember>, AppError> {
        repository::list_tenant_members(&self.0, tenant_id).await
    }
    async fn update_tenant_member(
        &self,
        member_id: Uuid,
        req: &UpdateTenantMember,
    ) -> Result<TenantMember, AppError> {
        repository::update_tenant_member(&self.0, member_id, req).await
    }
    async fn remove_tenant_member(&self, member_id: Uuid) -> Result<(), AppError> {
        repository::remove_tenant_member(&self.0, member_id).await
    }
    async fn get_tenant_member_for_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<TenantMember>, AppError> {
        repository::get_tenant_member_for_user(&self.0, tenant_id, user_id).await
    }
    async fn get_tenant_for_user(&self, user_id: Uuid) -> Result<Option<Tenant>, AppError> {
        repository::get_tenant_for_user(&self.0, user_id).await
    }

    // Wrapped Keys
    async fn create_wrapped_key(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        req: &CreateWrappedKey,
    ) -> Result<WrappedKey, AppError> {
        repository::create_wrapped_key(&self.0, tenant_id, user_id, req).await
    }
    async fn list_wrapped_keys(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<WrappedKey>, AppError> {
        repository::list_wrapped_keys(&self.0, tenant_id, user_id).await
    }
    async fn delete_wrapped_key(&self, key_id: Uuid) -> Result<(), AppError> {
        repository::delete_wrapped_key(&self.0, key_id).await
    }

    // ── Packages ──────────────────────────────────────────────────────────────
    async fn get_package_for_project(&self, project_id: Uuid) -> Result<Option<Package>, AppError> {
        repository::get_package_for_project(&self.0, project_id).await
    }

    async fn list_packages(&self) -> Result<Vec<Package>, AppError> {
        repository::list_packages(&self.0).await
    }

    async fn get_package_by_id(&self, id: Uuid) -> Result<Package, AppError> {
        repository::get_package_by_id(&self.0, id).await
    }

    async fn get_package_by_slug(&self, slug: &str) -> Result<Package, AppError> {
        repository::get_package_by_slug(&self.0, slug).await
    }

    async fn create_package(&self, req: &CreatePackage) -> Result<Package, AppError> {
        repository::create_package(&self.0, req).await
    }

    async fn update_package(&self, id: Uuid, req: &UpdatePackage) -> Result<Package, AppError> {
        repository::update_package(&self.0, id, req).await
    }

    async fn delete_package(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_package(&self.0, id).await
    }

    // ── Reports ──────────────────────────────────────────────────────────────
    async fn create_report(
        &self,
        project_id: Uuid,
        req: &CreateReport,
        created_by: Uuid,
    ) -> Result<Report, AppError> {
        repository::create_report(&self.0, project_id, req, created_by).await
    }
    async fn get_report_by_id(&self, id: Uuid) -> Result<Report, AppError> {
        repository::get_report_by_id(&self.0, id).await
    }
    async fn list_reports(
        &self,
        project_id: Uuid,
        filters: &ReportFilters,
    ) -> Result<Vec<Report>, AppError> {
        repository::list_reports(&self.0, project_id, filters).await
    }
    async fn count_reports(
        &self,
        project_id: Uuid,
        filters: &ReportFilters,
    ) -> Result<i64, AppError> {
        repository::count_reports(&self.0, project_id, filters).await
    }
    async fn update_report(&self, id: Uuid, req: &UpdateReport) -> Result<Report, AppError> {
        repository::update_report(&self.0, id, req).await
    }
    async fn get_report_by_task_id(&self, task_id: Uuid) -> Result<Option<Report>, AppError> {
        repository::get_report_by_task_id(&self.0, task_id).await
    }
    async fn delete_report(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_report(&self.0, id).await
    }

    // ── Task Logs ──────────────────────────────────────────────────────────
    async fn create_task_log(
        &self,
        project_id: Uuid,
        agent_id: Option<Uuid>,
        req: &CreateTaskLog,
    ) -> Result<TaskLog, AppError> {
        repository::create_task_log(&self.0, project_id, agent_id, req).await
    }
    async fn list_task_logs(
        &self,
        project_id: Uuid,
        filters: &TaskLogFilters,
    ) -> Result<Vec<TaskLogSummary>, AppError> {
        repository::list_task_logs(&self.0, project_id, filters).await
    }
    async fn count_task_logs(
        &self,
        project_id: Uuid,
        filters: &TaskLogFilters,
    ) -> Result<i64, AppError> {
        repository::count_task_logs(&self.0, project_id, filters).await
    }
    async fn get_task_log_by_id(&self, id: Uuid) -> Result<TaskLog, AppError> {
        repository::get_task_log_by_id(&self.0, id).await
    }

    // Event Observation Rules
    async fn create_event_observation_rule(
        &self,
        project_id: Uuid,
        req: &CreateEventObservationRule,
    ) -> Result<EventObservationRule, AppError> {
        repository::create_event_observation_rule(&self.0, project_id, req).await
    }
    async fn get_event_observation_rule(&self, id: Uuid) -> Result<EventObservationRule, AppError> {
        repository::get_event_observation_rule(&self.0, id).await
    }
    async fn list_event_observation_rules(
        &self,
        project_id: Uuid,
        filters: &EventObservationRuleFilters,
    ) -> Result<Vec<EventObservationRule>, AppError> {
        repository::list_event_observation_rules(&self.0, project_id, filters).await
    }
    async fn update_event_observation_rule(
        &self,
        id: Uuid,
        req: &UpdateEventObservationRule,
    ) -> Result<EventObservationRule, AppError> {
        repository::update_event_observation_rule(&self.0, id, req).await
    }
    async fn delete_event_observation_rule(&self, id: Uuid) -> Result<(), AppError> {
        repository::delete_event_observation_rule(&self.0, id).await
    }
    async fn find_matching_event_rules(
        &self,
        project_id: Uuid,
        event_kind: &str,
        event_source: &str,
        event_severity: Option<&str>,
    ) -> Result<Vec<EventObservationRule>, AppError> {
        repository::find_matching_rules(
            &self.0,
            project_id,
            event_kind,
            event_source,
            event_severity,
        )
        .await
    }
}
