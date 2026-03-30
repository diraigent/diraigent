//! OpenAPI specification for the Diraigent API.
//!
//! This module defines the complete OpenAPI 3.1 spec and provides
//! the Swagger UI endpoint.

use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::*;
use utoipa::{Modify, OpenApi};

use crate::models::*;
use crate::scoring::TaskScore;

/// OpenAPI document with all schemas registered.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Diraigent API",
        description = "AI-agent-first project management API. Built with Rust/Axum.\n\n\
            All endpoints under `/v1` require authentication via Bearer token (JWT) \
            or Agent API key (`dak_` prefix).",
        version = "0.1.0",
        license(name = "MIT"),
    ),
    servers(
        (url = "/", description = "Current server")
    ),
    modifiers(&SecurityAddon, &PathsAddon),
    components(
        schemas(
            // -- Agents --
            Agent, AgentRegistered, CreateAgent, UpdateAgent, HeartbeatRequest,
            // -- Packages --
            Package, PackageInfo, CreatePackage, UpdatePackage,
            // -- Projects --
            Project, ProjectResponse, CreateProject, UpdateProject, Pagination,
            // -- Tasks --
            Task, ScoredTask, TaskScore, TaskWithDecision, DecisionSummary,
            TaskSummaryForDecision,
            TaskDependency, TaskDependencyInfo, TaskDependencies,
            TaskUpdate, TaskComment, CreateTask, UpdateTask,
            TransitionTask, ClaimTask, DelegateTask,
            CreateTaskUpdate, CreateTaskComment, AddDependency,
            TaskFilters, TaskCostUpdate,
            BulkTransition, BulkDelegate, BulkDelete, BulkResult, BulkFailure,
            // -- Work --
            Work, TaskWork, WorkComment, WorkProgress, WorkStats, WorkSummary,
            CreateWork, UpdateWork, ReorderWorks, LinkTaskWork,
            BulkLinkTasks, CreateWorkComment, ReorderWorkTasks, WorkFilters,
            // -- Knowledge --
            Knowledge, CreateKnowledge, UpdateKnowledge, KnowledgeFilters,
            // -- Decisions --
            Decision, DecisionAlternative,
            CreateDecision, UpdateDecision, DecisionFilters,
            // -- Observations --
            Observation, CreateObservation, UpdateObservation,
            PromoteObservation, CleanupObservationsResult, ObservationFilters,
            // -- Playbooks --
            Playbook, CreatePlaybook, UpdatePlaybook, PlaybookFilters,
            // -- Events --
            Event, CreateEvent, EventFilters,
            // -- Integrations --
            Integration, AgentIntegration, CreateIntegration, UpdateIntegration,
            GrantAccess, IntegrationFilters,
            // -- Roles & Membership --
            Role, Membership, CreateRole, UpdateRole,
            CreateMembership, UpdateMembership,
            // -- Audit --
            AuditEntry, AuditFilters,
            // -- Context --
            AgentContext,
            // -- File Locks --
            FileLock, AcquireLocks,
            // -- Webhooks --
            Webhook, WebhookDelivery, WebhookDeadLetter,
            CreateWebhook, UpdateWebhook,
            // -- Search --
            SearchQuery, SearchResult, SearchResponse,
            // -- Metrics --
            MetricsQuery, ProjectMetrics, CostSummary, TaskCostRow,
            TaskSummary, DayCount, TokenDayCount, StateAvg,
            AgentMetrics, PlaybookMetrics,
            // -- Verifications --
            Verification, CreateVerification, UpdateVerification, VerificationFilters,
            // -- Changed Files --
            ChangedFile, ChangedFileSummary, CreateChangedFile, CreateChangedFiles,
            // -- Tenants --
            Tenant, TenantMember, WrappedKey,
            CreateTenant, UpdateTenant, AddTenantMember, UpdateTenantMember,
            CreateWrappedKey, TenantFilters,
            // -- Reports --
            Report, CreateReport, UpdateReport, ReportFilters, CompleteReport,
            // -- Task Logs --
            TaskLog, TaskLogSummary, CreateTaskLog, TaskLogFilters,
            // -- Step Templates --
            StepTemplate, CreateStepTemplate, UpdateStepTemplate, StepTemplateFilters,
            // -- Event Rules --
            EventObservationRule, CreateEventObservationRule,
            UpdateEventObservationRule, EventObservationRuleFilters,
            // -- Related Items --
            RelatedItem, RelatedItems,
            // -- Provider Configs --
            ProviderConfig, CreateProviderConfig, UpdateProviderConfig,
            ProviderConfigFilters, ResolvedProviderConfig,
            // -- CI --
            ForgejoIntegration, CiRun, CiJob, CiStep, CiRunFilters,
            CiRunWithJobs, CiJobWithSteps,
            CreateForgejoIntegration, ForgejoIntegrationResponse,
            GitHubIntegration, CreateGitHubIntegration, GitHubIntegrationResponse,
            // -- Dashboard --
            DashboardSummary, DashboardProjectSummary, DashboardQuery,
        )
    ),
    tags(
        (name = "projects", description = "Project CRUD and hierarchy"),
        (name = "tasks", description = "Task operations and state machine"),
        (name = "agents", description = "Agent registry and heartbeat"),
        (name = "work", description = "Work items (epics/features/milestones)"),
        (name = "knowledge", description = "Knowledge base entries"),
        (name = "decisions", description = "Architecture decision records"),
        (name = "observations", description = "Code smells, risks, and improvements"),
        (name = "playbooks", description = "Task lifecycle playbooks"),
        (name = "events", description = "Project events"),
        (name = "integrations", description = "External service integrations"),
        (name = "roles", description = "Role definitions"),
        (name = "members", description = "Project membership management"),
        (name = "audit", description = "Audit log"),
        (name = "webhooks", description = "Webhook management"),
        (name = "search", description = "Full-text search"),
        (name = "metrics", description = "Project metrics and analytics"),
        (name = "verifications", description = "Test/check verification records"),
        (name = "changed-files", description = "Task changed file tracking"),
        (name = "tenants", description = "Multi-tenant management"),
        (name = "reports", description = "Generated reports"),
        (name = "task-logs", description = "Task execution logs"),
        (name = "step-templates", description = "Reusable playbook step templates"),
        (name = "event-rules", description = "Event-to-observation rules"),
        (name = "provider-configs", description = "AI provider configurations"),
        (name = "ci", description = "CI/CD run tracking"),
        (name = "git", description = "Git operations"),
        (name = "context", description = "Agent operating context"),
        (name = "locks", description = "File lock management"),
        (name = "source", description = "Source code browsing"),
        (name = "files", description = "CLAUDE.md file management"),
        (name = "sse", description = "Server-sent events streams"),
        (name = "dashboard", description = "Dashboard summary"),
        (name = "account", description = "User account management"),
        (name = "health", description = "Health checks"),
    )
)]
pub struct ApiDoc;

/// Adds Bearer token security scheme.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_token",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("JWT token or Agent API key (prefix `dak_`)"))
                    .build(),
            ),
        );
    }
}

/// Adds all API paths programmatically.
struct PathsAddon;

impl Modify for PathsAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        register_all_paths(openapi);
    }
}

// ── Path Registration ──────────────────────────────────────────────────────

/// Compact route definition.
struct R {
    method: HttpMethod,
    path: &'static str,
    tag: &'static str,
    summary: &'static str,
    op_id: &'static str,
}

/// Helper to build a path parameter.
fn path_param(name: &str) -> path::Parameter {
    path::ParameterBuilder::new()
        .name(name)
        .parameter_in(path::ParameterIn::Path)
        .required(Required::True)
        .schema(Some(
            ObjectBuilder::new()
                .schema_type(schema::Type::String)
                .format(Some(SchemaFormat::Custom("uuid".into())))
                .build(),
        ))
        .build()
}

/// Extract `{param}` names from a path template.
fn extract_path_params(path: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        if let Some(end) = rest[start..].find('}') {
            params.push(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }
    params
}

fn add_route(openapi: &mut utoipa::openapi::OpenApi, r: &R) {
    let mut operation = path::OperationBuilder::new()
        .tag(r.tag)
        .summary(Some(r.summary))
        .operation_id(Some(r.op_id))
        .security(SecurityRequirement::new(
            "bearer_token",
            Vec::<String>::new(),
        ))
        .response("200", ResponseBuilder::new().description("Success").build())
        .response(
            "401",
            ResponseBuilder::new().description("Unauthorized").build(),
        );

    for param_name in extract_path_params(r.path) {
        operation = operation.parameter(path_param(&param_name));
    }

    let built_op = operation.build();

    match openapi.paths.paths.entry(r.path.to_string()) {
        std::collections::btree_map::Entry::Occupied(mut e) => {
            set_operation(e.get_mut(), &r.method, built_op);
        }
        std::collections::btree_map::Entry::Vacant(e) => {
            e.insert(path::PathItem::new(r.method.clone(), built_op));
        }
    }
}

fn set_operation(pi: &mut path::PathItem, method: &HttpMethod, op: path::Operation) {
    match method {
        HttpMethod::Get => pi.get = Some(op),
        HttpMethod::Post => pi.post = Some(op),
        HttpMethod::Put => pi.put = Some(op),
        HttpMethod::Delete => pi.delete = Some(op),
        HttpMethod::Patch => pi.patch = Some(op),
        HttpMethod::Head => pi.head = Some(op),
        HttpMethod::Options => pi.options = Some(op),
        HttpMethod::Trace => pi.trace = Some(op),
    }
}

/// Register all 228 API routes.
fn register_all_paths(openapi: &mut utoipa::openapi::OpenApi) {
    let routes: &[R] = &[
        // ── Health ──
        R {
            method: HttpMethod::Get,
            path: "/health/live",
            tag: "health",
            summary: "Liveness probe",
            op_id: "health_live",
        },
        R {
            method: HttpMethod::Get,
            path: "/health/ready",
            tag: "health",
            summary: "Readiness probe",
            op_id: "health_ready",
        },
        // ── Projects ──
        R {
            method: HttpMethod::Get,
            path: "/v1/",
            tag: "projects",
            summary: "List projects",
            op_id: "list_projects",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/",
            tag: "projects",
            summary: "Create project",
            op_id: "create_project",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}",
            tag: "projects",
            summary: "Get project",
            op_id: "get_project",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/{project_id}",
            tag: "projects",
            summary: "Update project",
            op_id: "update_project",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/{project_id}",
            tag: "projects",
            summary: "Delete project",
            op_id: "delete_project",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/by-slug/{slug}",
            tag: "projects",
            summary: "Get project by slug",
            op_id: "get_project_by_slug",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/children",
            tag: "projects",
            summary: "List child projects",
            op_id: "get_project_children",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tree",
            tag: "projects",
            summary: "Get project tree",
            op_id: "get_project_tree",
        },
        // ── Tasks ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tasks",
            tag: "tasks",
            summary: "List tasks",
            op_id: "list_tasks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/tasks",
            tag: "tasks",
            summary: "Create task",
            op_id: "create_task",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tasks/ready",
            tag: "tasks",
            summary: "List ready tasks (scored)",
            op_id: "list_ready_tasks",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tasks/blocked",
            tag: "tasks",
            summary: "List blocked task IDs",
            op_id: "list_blocked_task_ids",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tasks/flagged",
            tag: "tasks",
            summary: "List flagged task IDs",
            op_id: "list_flagged_task_ids",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tasks/with-blockers",
            tag: "tasks",
            summary: "List tasks with blocker updates",
            op_id: "list_tasks_with_blocker_updates",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/tasks/work-linked",
            tag: "tasks",
            summary: "List work-linked task IDs",
            op_id: "list_work_linked_task_ids",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/tasks/bulk/transition",
            tag: "tasks",
            summary: "Bulk transition tasks",
            op_id: "bulk_transition_tasks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/tasks/bulk/delegate",
            tag: "tasks",
            summary: "Bulk delegate tasks",
            op_id: "bulk_delegate_tasks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/tasks/bulk/delete",
            tag: "tasks",
            summary: "Bulk delete tasks",
            op_id: "bulk_delete_tasks",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}",
            tag: "tasks",
            summary: "Get task",
            op_id: "get_task",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/tasks/{task_id}",
            tag: "tasks",
            summary: "Update task",
            op_id: "update_task",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/tasks/{task_id}",
            tag: "tasks",
            summary: "Delete task",
            op_id: "delete_task",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/transition",
            tag: "tasks",
            summary: "Transition task state",
            op_id: "transition_task",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/claim",
            tag: "tasks",
            summary: "Claim task",
            op_id: "claim_task",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/release",
            tag: "tasks",
            summary: "Release task",
            op_id: "release_task",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/delegate",
            tag: "tasks",
            summary: "Delegate task",
            op_id: "delegate_task",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/dependencies",
            tag: "tasks",
            summary: "List task dependencies",
            op_id: "list_dependencies",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/dependencies",
            tag: "tasks",
            summary: "Add task dependency",
            op_id: "add_dependency",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/tasks/{task_id}/dependencies/{dep_id}",
            tag: "tasks",
            summary: "Remove task dependency",
            op_id: "remove_dependency",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/updates",
            tag: "tasks",
            summary: "List task updates",
            op_id: "list_task_updates",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/updates",
            tag: "tasks",
            summary: "Create task update",
            op_id: "create_task_update",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/comments",
            tag: "tasks",
            summary: "List task comments",
            op_id: "list_task_comments",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/comments",
            tag: "tasks",
            summary: "Create task comment",
            op_id: "create_task_comment",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/work",
            tag: "tasks",
            summary: "List task work items",
            op_id: "list_task_works",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/children",
            tag: "tasks",
            summary: "List task children",
            op_id: "list_task_children",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/related",
            tag: "tasks",
            summary: "Get related items",
            op_id: "get_related_items",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/cost",
            tag: "tasks",
            summary: "Record task cost",
            op_id: "record_task_cost",
        },
        // ── Changed Files ──
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/changed-files",
            tag: "changed-files",
            summary: "List changed files",
            op_id: "list_changed_files",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tasks/{task_id}/changed-files",
            tag: "changed-files",
            summary: "Create changed files",
            op_id: "create_changed_files",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tasks/{task_id}/changed-files/{file_id}",
            tag: "changed-files",
            summary: "Get changed file",
            op_id: "get_changed_file",
        },
        // ── Agents ──
        R {
            method: HttpMethod::Get,
            path: "/v1/agents",
            tag: "agents",
            summary: "List agents",
            op_id: "list_agents",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/agents",
            tag: "agents",
            summary: "Register agent",
            op_id: "register_agent",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/{agent_id}",
            tag: "agents",
            summary: "Get agent",
            op_id: "get_agent",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/agents/{agent_id}",
            tag: "agents",
            summary: "Update agent",
            op_id: "update_agent",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/agents/{agent_id}/heartbeat",
            tag: "agents",
            summary: "Agent heartbeat",
            op_id: "agent_heartbeat",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/{agent_id}/tasks",
            tag: "agents",
            summary: "List agent tasks",
            op_id: "list_agent_tasks",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/{agent_id}/memberships",
            tag: "agents",
            summary: "List agent memberships",
            op_id: "list_agent_memberships",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/{agent_id}/integrations",
            tag: "agents",
            summary: "List agent integrations",
            op_id: "agent_integrations",
        },
        // ── Context ──
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/{agent_id}/context/{project_id}",
            tag: "context",
            summary: "Get agent context",
            op_id: "get_context",
        },
        // ── Work ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/work",
            tag: "work",
            summary: "List work items",
            op_id: "list_works",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/work",
            tag: "work",
            summary: "Create work item",
            op_id: "create_work",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/work/counts",
            tag: "work",
            summary: "Work status counts",
            op_id: "work_status_counts",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/work/reorder",
            tag: "work",
            summary: "Reorder work items",
            op_id: "reorder_works",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/work/{work_id}/activate",
            tag: "work",
            summary: "Activate work item",
            op_id: "activate_work",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/work/{work_id}/tasks/reorder",
            tag: "work",
            summary: "Reorder work tasks",
            op_id: "reorder_work_tasks",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/work/{work_id}",
            tag: "work",
            summary: "Get work item",
            op_id: "get_work",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/work/{work_id}",
            tag: "work",
            summary: "Update work item",
            op_id: "update_work",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/work/{work_id}",
            tag: "work",
            summary: "Delete work item",
            op_id: "delete_work",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/work/{work_id}/children",
            tag: "work",
            summary: "List work children",
            op_id: "list_work_children",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/work/{work_id}/tasks",
            tag: "work",
            summary: "List work tasks",
            op_id: "list_work_tasks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/work/{work_id}/tasks",
            tag: "work",
            summary: "Link task to work",
            op_id: "link_task",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/work/{work_id}/tasks/bulk",
            tag: "work",
            summary: "Bulk link tasks",
            op_id: "bulk_link_tasks",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/work/{work_id}/tasks/{task_id}",
            tag: "work",
            summary: "Unlink task from work",
            op_id: "unlink_task",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/work/summaries",
            tag: "work",
            summary: "Bulk work summaries (progress + stats)",
            op_id: "get_bulk_work_summaries",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/work/{work_id}/progress",
            tag: "work",
            summary: "Get work progress",
            op_id: "get_work_progress",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/work/{work_id}/stats",
            tag: "work",
            summary: "Get work stats",
            op_id: "get_work_stats",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/work/{work_id}/comments",
            tag: "work",
            summary: "List work comments",
            op_id: "list_work_comments",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/work/{work_id}/comments",
            tag: "work",
            summary: "Create work comment",
            op_id: "create_work_comment",
        },
        // ── Knowledge ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/knowledge",
            tag: "knowledge",
            summary: "List knowledge",
            op_id: "list_knowledge",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/knowledge",
            tag: "knowledge",
            summary: "Create knowledge",
            op_id: "create_knowledge",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/knowledge/{id}",
            tag: "knowledge",
            summary: "Get knowledge entry",
            op_id: "get_knowledge",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/knowledge/{id}",
            tag: "knowledge",
            summary: "Update knowledge",
            op_id: "update_knowledge",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/knowledge/{id}",
            tag: "knowledge",
            summary: "Delete knowledge",
            op_id: "delete_knowledge",
        },
        // ── Decisions ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/decisions",
            tag: "decisions",
            summary: "List decisions",
            op_id: "list_decisions",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/decisions",
            tag: "decisions",
            summary: "Create decision",
            op_id: "create_decision",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/decisions/{id}",
            tag: "decisions",
            summary: "Get decision",
            op_id: "get_decision",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/decisions/{id}",
            tag: "decisions",
            summary: "Update decision",
            op_id: "update_decision",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/decisions/{id}",
            tag: "decisions",
            summary: "Delete decision",
            op_id: "delete_decision",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/decisions/{id}/tasks",
            tag: "decisions",
            summary: "List linked tasks",
            op_id: "list_decision_tasks",
        },
        // ── Observations ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/observations",
            tag: "observations",
            summary: "List observations",
            op_id: "list_observations",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/observations",
            tag: "observations",
            summary: "Create observation",
            op_id: "create_observation",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/observations/cleanup",
            tag: "observations",
            summary: "Cleanup observations",
            op_id: "cleanup_observations",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/observations/{id}",
            tag: "observations",
            summary: "Get observation",
            op_id: "get_observation",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/observations/{id}",
            tag: "observations",
            summary: "Update observation",
            op_id: "update_observation",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/observations/{id}",
            tag: "observations",
            summary: "Delete observation",
            op_id: "delete_observation",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/observations/{id}/dismiss",
            tag: "observations",
            summary: "Dismiss observation",
            op_id: "dismiss_observation",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/observations/{id}/promote",
            tag: "observations",
            summary: "Promote to task",
            op_id: "promote_observation",
        },
        // ── Playbooks ──
        R {
            method: HttpMethod::Get,
            path: "/v1/playbooks",
            tag: "playbooks",
            summary: "List playbooks",
            op_id: "list_playbooks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/playbooks",
            tag: "playbooks",
            summary: "Create playbook",
            op_id: "create_playbook",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/playbooks/{id}",
            tag: "playbooks",
            summary: "Get playbook",
            op_id: "get_playbook",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/playbooks/{id}",
            tag: "playbooks",
            summary: "Update playbook",
            op_id: "update_playbook",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/playbooks/{id}",
            tag: "playbooks",
            summary: "Delete playbook",
            op_id: "delete_playbook",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/playbooks/{id}/sync",
            tag: "playbooks",
            summary: "Sync with parent",
            op_id: "sync_playbook",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/git-strategies",
            tag: "playbooks",
            summary: "List git strategies",
            op_id: "git_strategies",
        },
        // ── Events ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/events",
            tag: "events",
            summary: "List events",
            op_id: "list_events",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/events",
            tag: "events",
            summary: "Create event",
            op_id: "create_event",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/events/recent",
            tag: "events",
            summary: "List recent events",
            op_id: "recent_events",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/events/{id}",
            tag: "events",
            summary: "Get event",
            op_id: "get_event",
        },
        // ── Event Rules ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/event-rules",
            tag: "event-rules",
            summary: "List event rules",
            op_id: "list_event_rules",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/event-rules",
            tag: "event-rules",
            summary: "Create event rule",
            op_id: "create_event_rule",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/event-rules/{id}",
            tag: "event-rules",
            summary: "Get event rule",
            op_id: "get_event_rule",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/event-rules/{id}",
            tag: "event-rules",
            summary: "Update event rule",
            op_id: "update_event_rule",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/event-rules/{id}",
            tag: "event-rules",
            summary: "Delete event rule",
            op_id: "delete_event_rule",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/event-rules/{id}/toggle",
            tag: "event-rules",
            summary: "Toggle event rule",
            op_id: "toggle_event_rule",
        },
        // ── Integrations ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/integrations",
            tag: "integrations",
            summary: "List integrations",
            op_id: "list_integrations",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/integrations",
            tag: "integrations",
            summary: "Create integration",
            op_id: "create_integration",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/integrations/forgejo",
            tag: "ci",
            summary: "Register Forgejo integration",
            op_id: "register_forgejo_integration",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/integrations/github",
            tag: "ci",
            summary: "Register GitHub integration",
            op_id: "register_github_integration",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/integrations/{id}",
            tag: "integrations",
            summary: "Get integration",
            op_id: "get_integration",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/integrations/{id}",
            tag: "integrations",
            summary: "Update integration",
            op_id: "update_integration",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/integrations/{id}",
            tag: "integrations",
            summary: "Delete integration",
            op_id: "delete_integration",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/integrations/{id}/access",
            tag: "integrations",
            summary: "List integration access",
            op_id: "list_integration_access",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/integrations/{id}/access",
            tag: "integrations",
            summary: "Grant integration access",
            op_id: "grant_integration_access",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/integrations/{id}/access/{agent_id}",
            tag: "integrations",
            summary: "Revoke integration access",
            op_id: "revoke_integration_access",
        },
        // ── Roles ──
        R {
            method: HttpMethod::Get,
            path: "/v1/roles",
            tag: "roles",
            summary: "List roles",
            op_id: "list_roles",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/roles",
            tag: "roles",
            summary: "Create role",
            op_id: "create_role",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/roles/{role_id}",
            tag: "roles",
            summary: "Get role",
            op_id: "get_role",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/roles/{role_id}",
            tag: "roles",
            summary: "Update role",
            op_id: "update_role",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/roles/{role_id}",
            tag: "roles",
            summary: "Delete role",
            op_id: "delete_role",
        },
        // ── Members ──
        R {
            method: HttpMethod::Get,
            path: "/v1/members",
            tag: "members",
            summary: "List members",
            op_id: "list_members",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/members",
            tag: "members",
            summary: "Create membership",
            op_id: "create_membership",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/members/{membership_id}",
            tag: "members",
            summary: "Get membership",
            op_id: "get_membership",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/members/{membership_id}",
            tag: "members",
            summary: "Update membership",
            op_id: "update_membership",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/members/{membership_id}",
            tag: "members",
            summary: "Remove membership",
            op_id: "remove_membership",
        },
        // ── Audit ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/audit",
            tag: "audit",
            summary: "List audit log",
            op_id: "list_audit",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/audit/{entity_type}/{entity_id}",
            tag: "audit",
            summary: "Entity history",
            op_id: "entity_history",
        },
        // ── Webhooks ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/webhooks",
            tag: "webhooks",
            summary: "List webhooks",
            op_id: "list_webhooks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/webhooks",
            tag: "webhooks",
            summary: "Create webhook",
            op_id: "create_webhook",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/webhooks/{id}",
            tag: "webhooks",
            summary: "Get webhook",
            op_id: "get_webhook",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/webhooks/{id}",
            tag: "webhooks",
            summary: "Update webhook",
            op_id: "update_webhook",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/webhooks/{id}",
            tag: "webhooks",
            summary: "Delete webhook",
            op_id: "delete_webhook",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/webhooks/{id}/deliveries",
            tag: "webhooks",
            summary: "List deliveries",
            op_id: "list_deliveries",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/webhooks/{id}/dead-letters",
            tag: "webhooks",
            summary: "List dead letters",
            op_id: "list_dead_letters",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/webhooks/{id}/test",
            tag: "webhooks",
            summary: "Test webhook",
            op_id: "test_webhook",
        },
        // ── Locks ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/locks",
            tag: "locks",
            summary: "List file locks",
            op_id: "list_locks",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/locks",
            tag: "locks",
            summary: "Acquire file locks",
            op_id: "acquire_locks",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/{project_id}/locks/{task_id}",
            tag: "locks",
            summary: "Release file locks",
            op_id: "release_locks",
        },
        // ── Search ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/search",
            tag: "search",
            summary: "Search project",
            op_id: "search",
        },
        // ── Metrics ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/metrics",
            tag: "metrics",
            summary: "Get project metrics",
            op_id: "get_metrics",
        },
        // ── Verifications ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/verifications",
            tag: "verifications",
            summary: "List verifications",
            op_id: "list_verifications",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/verifications",
            tag: "verifications",
            summary: "Create verification",
            op_id: "create_verification",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/verifications/{id}",
            tag: "verifications",
            summary: "Get verification",
            op_id: "get_verification",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/verifications/{id}",
            tag: "verifications",
            summary: "Update verification",
            op_id: "update_verification",
        },
        // ── Git ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/git/branches",
            tag: "git",
            summary: "List branches",
            op_id: "list_branches",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/git/main-status",
            tag: "git",
            summary: "Main branch status",
            op_id: "main_status",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/git/push",
            tag: "git",
            summary: "Push branch",
            op_id: "push_branch",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/git/push-main",
            tag: "git",
            summary: "Push main branch",
            op_id: "push_main",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/git/release",
            tag: "git",
            summary: "Create release",
            op_id: "release",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/git/resolve-and-push-main",
            tag: "git",
            summary: "Resolve conflicts and push main",
            op_id: "resolve_and_push_main",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/git/resolve-task-branch/{task_id}",
            tag: "git",
            summary: "Resolve task branch",
            op_id: "resolve_task_branch",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/git/revert-task/{task_id}",
            tag: "git",
            summary: "Revert task",
            op_id: "revert_task",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/git/task-branch/{task_id}",
            tag: "git",
            summary: "Task branch status",
            op_id: "task_branch_status",
        },
        // ── Source ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/source/tree",
            tag: "source",
            summary: "Source tree",
            op_id: "source_tree",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/source/blob",
            tag: "source",
            summary: "Source blob",
            op_id: "source_blob",
        },
        // ── Files ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/claude-md",
            tag: "files",
            summary: "Get CLAUDE.md",
            op_id: "get_claude_md",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/{project_id}/claude-md",
            tag: "files",
            summary: "Update CLAUDE.md",
            op_id: "put_claude_md",
        },
        // ── SSE ──
        R {
            method: HttpMethod::Get,
            path: "/v1/review/stream",
            tag: "sse",
            summary: "Review SSE stream",
            op_id: "review_stream",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/review/stream/ticket",
            tag: "sse",
            summary: "Issue review stream ticket",
            op_id: "issue_review_ticket",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/stream",
            tag: "sse",
            summary: "Agent status SSE stream",
            op_id: "agent_stream",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/agents/stream/ticket",
            tag: "sse",
            summary: "Issue agent stream ticket",
            op_id: "issue_agent_ticket",
        },
        // ── Chat ──
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/chat",
            tag: "projects",
            summary: "Chat with project AI",
            op_id: "chat",
        },
        // ── Logs ──
        R {
            method: HttpMethod::Get,
            path: "/v1/logs",
            tag: "metrics",
            summary: "Query logs",
            op_id: "query_logs",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/logs/labels",
            tag: "metrics",
            summary: "Log labels",
            op_id: "log_labels",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/logs/labels/{name}/values",
            tag: "metrics",
            summary: "Log label values",
            op_id: "log_label_values",
        },
        // ── Tenants ──
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants",
            tag: "tenants",
            summary: "List tenants",
            op_id: "list_tenants",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tenants",
            tag: "tenants",
            summary: "Create tenant",
            op_id: "create_tenant",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/me",
            tag: "tenants",
            summary: "Get my tenant",
            op_id: "get_my_tenant",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/by-slug/{slug}",
            tag: "tenants",
            summary: "Get tenant by slug",
            op_id: "get_tenant_by_slug",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/{tenant_id}",
            tag: "tenants",
            summary: "Get tenant",
            op_id: "get_tenant",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/tenants/{tenant_id}",
            tag: "tenants",
            summary: "Update tenant",
            op_id: "update_tenant",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/tenants/{tenant_id}",
            tag: "tenants",
            summary: "Delete tenant",
            op_id: "delete_tenant",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/{tenant_id}/members",
            tag: "tenants",
            summary: "List tenant members",
            op_id: "list_tenant_members",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tenants/{tenant_id}/members",
            tag: "tenants",
            summary: "Add tenant member",
            op_id: "add_tenant_member",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/tenants/{tenant_id}/members/{member_id}",
            tag: "tenants",
            summary: "Update tenant member",
            op_id: "update_tenant_member",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/tenants/{tenant_id}/members/{member_id}",
            tag: "tenants",
            summary: "Remove tenant member",
            op_id: "remove_tenant_member",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tenants/{tenant_id}/encryption/init",
            tag: "tenants",
            summary: "Initialize encryption",
            op_id: "init_encryption",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tenants/{tenant_id}/encryption/unlock",
            tag: "tenants",
            summary: "Unlock encryption",
            op_id: "unlock_encryption",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tenants/{tenant_id}/encryption/rotate",
            tag: "tenants",
            summary: "Rotate encryption keys",
            op_id: "rotate_keys",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/{tenant_id}/encryption/salt",
            tag: "tenants",
            summary: "Get encryption salt",
            op_id: "get_encryption_salt",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/{tenant_id}/encryption/dek",
            tag: "tenants",
            summary: "Get DEK for orchestra",
            op_id: "get_dek",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/tenants/{tenant_id}/members/{user_id}/keys",
            tag: "tenants",
            summary: "List wrapped keys",
            op_id: "list_keys",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/tenants/{tenant_id}/members/{user_id}/keys",
            tag: "tenants",
            summary: "Create wrapped key",
            op_id: "create_key",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/tenants/{tenant_id}/keys/{key_id}",
            tag: "tenants",
            summary: "Delete wrapped key",
            op_id: "delete_key",
        },
        // ── Settings ──
        R {
            method: HttpMethod::Get,
            path: "/v1/settings",
            tag: "projects",
            summary: "Get settings",
            op_id: "get_settings",
        },
        // ── Step Templates ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/step-templates",
            tag: "step-templates",
            summary: "List step templates",
            op_id: "list_step_templates",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/step-templates",
            tag: "step-templates",
            summary: "Create step template",
            op_id: "create_step_template",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/step-templates/{id}",
            tag: "step-templates",
            summary: "Get step template",
            op_id: "get_step_template",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/{project_id}/step-templates/{id}",
            tag: "step-templates",
            summary: "Update step template",
            op_id: "update_step_template",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/{project_id}/step-templates/{id}",
            tag: "step-templates",
            summary: "Delete step template",
            op_id: "delete_step_template",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/step-templates/{id}/fork",
            tag: "step-templates",
            summary: "Fork step template",
            op_id: "fork_step_template",
        },
        // ── Provider Configs ──
        R {
            method: HttpMethod::Get,
            path: "/v1/providers",
            tag: "provider-configs",
            summary: "List global providers",
            op_id: "list_global_providers",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/providers",
            tag: "provider-configs",
            summary: "Create global provider",
            op_id: "create_global_provider",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/providers/{id}",
            tag: "provider-configs",
            summary: "Get provider config",
            op_id: "get_provider_config",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/providers/{id}",
            tag: "provider-configs",
            summary: "Update provider config",
            op_id: "update_provider_config",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/providers/{id}",
            tag: "provider-configs",
            summary: "Delete provider config",
            op_id: "delete_provider_config",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/providers",
            tag: "provider-configs",
            summary: "List project providers",
            op_id: "list_project_providers",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/providers",
            tag: "provider-configs",
            summary: "Create project provider",
            op_id: "create_project_provider",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/providers/resolve/{provider}",
            tag: "provider-configs",
            summary: "Resolve provider config",
            op_id: "resolve_provider_config",
        },
        // ── Reports ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/reports",
            tag: "reports",
            summary: "List reports",
            op_id: "list_reports",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/reports",
            tag: "reports",
            summary: "Create report",
            op_id: "create_report",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/reports/{id}/complete",
            tag: "reports",
            summary: "Complete report",
            op_id: "complete_report",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/reports/{id}",
            tag: "reports",
            summary: "Get report",
            op_id: "get_report",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/reports/{id}",
            tag: "reports",
            summary: "Update report",
            op_id: "update_report",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/reports/{id}",
            tag: "reports",
            summary: "Delete report",
            op_id: "delete_report",
        },
        // ── Task Logs ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/task-logs",
            tag: "task-logs",
            summary: "List task logs",
            op_id: "list_task_logs",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/task-logs",
            tag: "task-logs",
            summary: "Create task log",
            op_id: "create_task_log",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/task-logs/{id}",
            tag: "task-logs",
            summary: "Get task log",
            op_id: "get_task_log",
        },
        // ── Packages ──
        R {
            method: HttpMethod::Get,
            path: "/v1/packages",
            tag: "projects",
            summary: "List packages",
            op_id: "list_packages",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/packages",
            tag: "projects",
            summary: "Create package",
            op_id: "create_package",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/packages/{id}",
            tag: "projects",
            summary: "Get package",
            op_id: "get_package",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/packages/{id}",
            tag: "projects",
            summary: "Update package",
            op_id: "update_package",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/packages/{id}",
            tag: "projects",
            summary: "Delete package",
            op_id: "delete_package",
        },
        // ── CI ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/ci/runs",
            tag: "ci",
            summary: "List CI runs",
            op_id: "list_ci_runs",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/ci/runs/{run_id}",
            tag: "ci",
            summary: "Get CI run",
            op_id: "get_ci_run",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/ci/runs/{run_id}/jobs/{job_id}",
            tag: "ci",
            summary: "Get CI job",
            op_id: "get_ci_job",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/forgejo/sync",
            tag: "ci",
            summary: "Sync Forgejo runs",
            op_id: "sync_forgejo_runs",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/{project_id}/github/sync",
            tag: "ci",
            summary: "Sync GitHub runs",
            op_id: "sync_github_runs",
        },
        // ── Webhooks (incoming) ──
        R {
            method: HttpMethod::Post,
            path: "/v1/webhooks/forgejo/{integration_id}",
            tag: "webhooks",
            summary: "Receive Forgejo webhook",
            op_id: "receive_forgejo_webhook",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/webhooks/github/{integration_id}",
            tag: "webhooks",
            summary: "Receive GitHub webhook",
            op_id: "receive_github_webhook",
        },
        R {
            method: HttpMethod::Post,
            path: "/v1/webhooks/authentik",
            tag: "webhooks",
            summary: "Receive Authentik webhook",
            op_id: "receive_authentik_webhook",
        },
        // ── Account ──
        R {
            method: HttpMethod::Get,
            path: "/v1/account",
            tag: "account",
            summary: "Get account",
            op_id: "get_account",
        },
        R {
            method: HttpMethod::Delete,
            path: "/v1/account",
            tag: "account",
            summary: "Delete account",
            op_id: "delete_account",
        },
        R {
            method: HttpMethod::Get,
            path: "/v1/account/export",
            tag: "account",
            summary: "Export account data",
            op_id: "export_account_data",
        },
        // ── Dashboard ──
        R {
            method: HttpMethod::Get,
            path: "/v1/dashboard/summary",
            tag: "dashboard",
            summary: "Dashboard summary",
            op_id: "get_dashboard_summary",
        },
        // ── Scratchpad ──
        R {
            method: HttpMethod::Get,
            path: "/v1/{project_id}/scratchpad",
            tag: "projects",
            summary: "Get scratchpad",
            op_id: "get_scratchpad",
        },
        R {
            method: HttpMethod::Put,
            path: "/v1/{project_id}/scratchpad",
            tag: "projects",
            summary: "Upsert scratchpad",
            op_id: "upsert_scratchpad",
        },
        // ── WebSocket ──
        R {
            method: HttpMethod::Get,
            path: "/v1/agents/{agent_id}/ws",
            tag: "agents",
            summary: "Agent WebSocket",
            op_id: "ws_handler",
        },
        // ── Config ──
        R {
            method: HttpMethod::Get,
            path: "/v1/config",
            tag: "health",
            summary: "Get API config",
            op_id: "get_config",
        },
    ];

    for r in routes {
        add_route(openapi, r);
    }
}
