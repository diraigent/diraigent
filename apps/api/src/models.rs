use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Deserialise a double-Option so that:
///   - missing field → `None`          (don't change)
///   - `"field": null` → `Some(None)`  (clear the value)
///   - `"field": "..."` → `Some(Some(v))` (set the value)
fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

// ── Valid Values ──

/// Fallback task kinds used when a project has no package assigned.
pub const TASK_KINDS: &[&str] = &[
    "feature", "bug", "refactor", "docs", "test", "research", "chore", "spike",
];
pub const AGENT_STATUSES: &[&str] = &["idle", "working", "offline", "revoked"];
pub const UPDATE_KINDS: &[&str] = &[
    "progress", "blocker", "question", "artifact", "review", "note",
];
pub const WORK_STATUSES: &[&str] = &[
    "active",
    "achieved",
    "abandoned",
    "paused",
    "ready",
    "processing",
];
pub const WORK_INTENT_TYPES: &[&str] =
    &["complex", "simple", "hotfix", "investigation", "refactor"];
pub const WORK_TYPES: &[&str] = &["epic", "feature", "milestone", "sprint", "initiative"];
pub const KNOWLEDGE_CATEGORIES: &[&str] = &[
    "architecture",
    "convention",
    "pattern",
    "anti_pattern",
    "setup",
    "general",
];
pub const DECISION_STATUSES: &[&str] = &[
    "proposed",
    "accepted",
    "rejected",
    "superseded",
    "deprecated",
];
pub const OBSERVATION_KINDS: &[&str] = &[
    "insight",
    "risk",
    "opportunity",
    "smell",
    "inconsistency",
    "improvement",
];
pub const OBSERVATION_SEVERITIES: &[&str] = &["info", "low", "medium", "high", "critical"];
pub const OBSERVATION_STATUSES: &[&str] = &["open", "acknowledged", "acted_on", "dismissed"];
pub const OBSERVATION_SOURCES: &[&str] = &[
    "dream",
    "implement",
    "review",
    "worker",
    "log_monitor",
    "manual",
    "event_trigger",
];
pub const EVENT_KINDS: &[&str] = &[
    "ci", "deploy", "error", "merge", "release", "alert", "custom",
];
pub const EVENT_SEVERITIES: &[&str] = &["info", "warning", "error", "critical"];
pub const INTEGRATION_KINDS: &[&str] = &[
    "logging",
    "tracing",
    "metrics",
    "git",
    "ci",
    "messaging",
    "monitoring",
    "storage",
    "database",
    "custom",
];
pub const AUTH_TYPES: &[&str] = &["none", "token", "basic", "header", "oauth"];
pub const AUTHORITIES: &[&str] = &[
    "execute", "delegate", "review", "create", "decide", "manage",
];
pub const REPORT_STATUSES: &[&str] = &["pending", "in_progress", "completed", "failed"];
pub const REPORT_KINDS: &[&str] = &[
    "security",
    "component",
    "architecture",
    "performance",
    "custom",
];
pub const MEMBERSHIP_STATUSES: &[&str] = &["active", "inactive", "suspended"];

// ── State Machine ──

/// Lifecycle states are the fixed states in the task state machine.
/// Active step states (e.g. "implement", "review", "dream") come from the
/// task's playbook and are stored as free-form strings in the `state` column.
///
/// State machine:
///   backlog    → ready, cancelled
///   ready      → <step_name> (via claim), cancelled
///   <step>     → done (final step), wait:<next> (more steps), ready (release), cancelled
///   wait:<next> → <next> (via claim), cancelled
///   done       → backlog (reopen), human_review
///   cancelled  → backlog (reopen)
///
/// "human_review" is a playbook step name, not a lifecycle state.
pub fn is_lifecycle_state(s: &str) -> bool {
    matches!(s, "backlog" | "ready" | "done" | "cancelled") || s.starts_with("wait:")
}

/// Returns true if the state is a `wait:<step>` inter-step state.
pub fn is_wait_state(s: &str) -> bool {
    s.starts_with("wait:")
}

/// Extract the next step name from a `wait:<step>` state.
pub fn wait_target(s: &str) -> Option<&str> {
    s.strip_prefix("wait:")
}

/// Validate whether a state transition is allowed.
/// Lifecycle states have fixed rules; any non-lifecycle string is treated
/// as a playbook step name (active state).
pub fn can_transition(current: &str, target: &str) -> bool {
    match current {
        "backlog" => matches!(target, "ready" | "cancelled"),
        "ready" => {
            // ready → any step name, or back to backlog/cancelled
            !is_lifecycle_state(target) || matches!(target, "backlog" | "cancelled")
        }
        "done" => {
            // done is terminal — reopen to backlog, or move to human_review
            target == "backlog" || target == "human_review"
        }
        "cancelled" => target == "backlog",
        _ if is_wait_state(current) => {
            // wait:<next> → the named step (via claim) or cancelled
            let next = wait_target(current).unwrap_or("");
            target == next || target == "cancelled"
        }
        _ => {
            // Current state is a step name (e.g. implement, review, human_review)
            // Can go to done (final), wait:<next> (pipeline), ready (release), or cancelled
            matches!(target, "done" | "ready" | "cancelled") || is_wait_state(target)
        }
    }
}

// ── Domain Models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: String,
    pub metadata: serde_json::Value,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub owner_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// SHA-256 hash of the agent's API key. Never serialized to clients.
    #[serde(skip_serializing)]
    #[sqlx(default)]
    pub api_key_hash: Option<String>,
}

/// Response returned when registering a new agent.
/// Includes the plaintext API key (shown only once).
#[derive(Debug, Serialize)]
pub struct AgentRegistered {
    #[serde(flatten)]
    pub agent: Agent,
    /// Plaintext API key — store this securely, it cannot be retrieved again.
    pub api_key: String,
}

// ── Package ──

/// A package defines the allowed domain taxonomy for a project.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Package {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub is_builtin: bool,
    pub allowed_task_kinds: Vec<String>,
    pub allowed_knowledge_categories: Vec<String>,
    pub allowed_observation_kinds: Vec<String>,
    pub allowed_event_kinds: Vec<String>,
    pub allowed_integration_kinds: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Slim package representation embedded in project responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

impl From<Package> for PackageInfo {
    fn from(p: Package) -> Self {
        PackageInfo {
            id: p.id,
            slug: p.slug,
            name: p.name,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePackage {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub allowed_task_kinds: Option<Vec<String>>,
    pub allowed_knowledge_categories: Option<Vec<String>>,
    pub allowed_observation_kinds: Option<Vec<String>>,
    pub allowed_event_kinds: Option<Vec<String>>,
    pub allowed_integration_kinds: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePackage {
    /// New slug — only permitted for non-built-in packages.
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_task_kinds: Option<Vec<String>>,
    pub allowed_knowledge_categories: Option<Vec<String>>,
    pub allowed_observation_kinds: Option<Vec<String>>,
    pub allowed_event_kinds: Option<Vec<String>>,
    pub allowed_integration_kinds: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub default_playbook_id: Option<Uuid>,
    /// FK to diraigent.package — determines which domain enum values are valid
    /// for tasks, knowledge, observations, events, and integrations in this project.
    pub package_id: Option<Uuid>,
    pub repo_url: Option<String>,
    /// Legacy field — use `git_root` for new projects.
    /// Kept for backward compatibility; populated from `git_root` on creation.
    pub repo_path: Option<String>,
    pub default_branch: String,
    pub service_name: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Git topology mode: "monorepo" | "standalone" | "none".
    ///
    /// - monorepo  : project lives inside a larger git repo
    /// - standalone: project IS the git repo
    /// - none      : no git repository
    pub git_mode: String,
    /// Path to the git repository root (where .git lives), relative to PROJECTS_PATH.
    /// Required for all git modes (standalone and monorepo). Null for git-free projects.
    pub git_root: Option<String>,
    /// Subpath within `git_root` for the project directory.
    /// Only present for monorepo mode. Null for standalone and git-free projects.
    pub project_root: Option<String>,
    /// The tenant that owns this project.
    pub tenant_id: Uuid,
}

/// Project with a resolved absolute filesystem path and package info.
///
/// When `PROJECTS_PATH` is configured, `resolved_path` is the absolute path
/// to the project's root directory: `PROJECTS_PATH / git_root / project_root`
/// (null path components are skipped).  Falls back to `repo_path` when both
/// fields are null (backward compatibility).
/// - `git_root` = git repo root directory (required for all git modes)
/// - `project_root` = subpath within git_root (monorepo only)
#[derive(Debug, Clone, Serialize)]
pub struct ProjectResponse {
    #[serde(flatten)]
    pub project: Project,
    /// Absolute filesystem path to the project root, if `PROJECTS_PATH` is set.
    pub resolved_path: Option<String>,
    /// Absolute filesystem path to the git repository root, if `PROJECTS_PATH`
    /// is set and the project has a git mode other than "none".
    pub git_resolved_path: Option<String>,
    /// The resolved package (id, slug, name) for this project.
    pub package: Option<PackageInfo>,
}

impl ProjectResponse {
    pub fn new(
        project: Project,
        projects_path: Option<&PathBuf>,
        package: Option<PackageInfo>,
    ) -> Self {
        let (resolved_path, git_resolved_path) = if project.git_mode == "none" {
            (None, None)
        } else if let Some(base) = projects_path {
            // git_resolved_path = PROJECTS_PATH / git_root  (the directory where .git lives)
            let git_base = if let Some(ref root) = project.git_root {
                base.join(root)
            } else {
                base.clone()
            };

            // resolved_path = git_base / project_root
            // project_root is the monorepo subpath (optional); if absent fall back to repo_path (legacy)
            let resolved = if let Some(ref pr) = project.project_root {
                if pr.is_empty() {
                    Some(git_base.to_string_lossy().into_owned())
                } else {
                    Some(git_base.join(pr).to_string_lossy().into_owned())
                }
            } else {
                // Backward compat: repo_path was relative to PROJECTS_PATH directly
                project
                    .repo_path
                    .as_ref()
                    .map(|legacy| base.join(legacy).to_string_lossy().into_owned())
            };

            let git_rp = if project.git_root.is_some() || project.project_root.is_some() {
                Some(git_base.to_string_lossy().into_owned())
            } else {
                resolved.clone()
            };

            (resolved, git_rp)
        } else if let Some(ref git_root) = project.git_root {
            // No PROJECTS_PATH — if git_root is an absolute path, use it directly.
            // This supports standalone deployments where git_root stores the full
            // filesystem path to the git repository.
            let git_root_path = PathBuf::from(git_root);
            if git_root_path.is_absolute() {
                let resolved = if let Some(ref pr) = project.project_root {
                    if pr.is_empty() {
                        Some(git_root_path.to_string_lossy().into_owned())
                    } else {
                        Some(git_root_path.join(pr).to_string_lossy().into_owned())
                    }
                } else {
                    Some(git_root_path.to_string_lossy().into_owned())
                };
                let git_rp = Some(git_root_path.to_string_lossy().into_owned());
                (resolved, git_rp)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        Self {
            project,
            resolved_path,
            git_resolved_path,
            package,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub number: i64,
    pub title: String,
    pub kind: String,
    pub state: String,
    pub urgent: bool,
    pub context: serde_json::Value,
    pub assigned_agent_id: Option<Uuid>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub required_capabilities: Vec<String>,
    pub assigned_role_id: Option<Uuid>,
    pub delegated_by: Option<Uuid>,
    pub delegated_at: Option<DateTime<Utc>>,
    pub playbook_id: Option<Uuid>,
    pub playbook_step: Option<i32>,
    /// FK to the decision that originated this task (nullable).
    pub decision_id: Option<Uuid>,
    /// FK to a parent task for plan decomposition (nullable, self-referencing).
    pub parent_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Timestamp when the task's changes were reverted from the default branch.
    pub reverted_at: Option<DateTime<Utc>>,
    /// User-toggleable flag (bookmark) for tracking tasks of interest.
    pub flagged: bool,
    /// File paths this task intends to modify — used by the orchestra
    /// to detect branch overlap and serialize conflicting work.
    pub file_scope: Vec<String>,
    /// Accumulated LLM input tokens across all completed steps.
    pub input_tokens: i64,
    /// Accumulated LLM output tokens across all completed steps.
    pub output_tokens: i64,
    /// Accumulated LLM cost in USD across all completed steps.
    pub cost_usd: f64,
    /// Timestamp when the task entered its current state — used for staleness scoring.
    pub state_entered_at: DateTime<Utc>,
}

/// Task with an attached composite score, returned by the ready-tasks endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredTask {
    #[serde(flatten)]
    pub task: Task,
    /// Composite score used for ordering. Higher = more important.
    pub score: f64,
    /// Per-component score breakdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_components: Option<crate::scoring::TaskScore>,
}

/// Lightweight decision summary embedded in task responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionSummary {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    /// First 300 chars of rationale, if set.
    pub rationale_excerpt: Option<String>,
}

/// Task response enriched with optional originating-decision summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWithDecision {
    #[serde(flatten)]
    pub task: Task,
    pub decision: Option<DecisionSummary>,
}

/// Task summary used in decision detail lists (avoids circular nesting).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskSummaryForDecision {
    pub id: Uuid,
    pub number: i64,
    pub title: String,
    pub kind: String,
    pub state: String,
    pub urgent: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskDependency {
    pub task_id: Uuid,
    pub depends_on: Uuid,
}

/// Enriched dependency with title and state of the referenced task.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskDependencyInfo {
    pub task_id: Uuid,
    pub depends_on: Uuid,
    pub title: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependencies {
    pub depends_on: Vec<TaskDependencyInfo>,
    pub blocks: Vec<TaskDependencyInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskUpdate {
    pub id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Task Comments ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskComment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Request DTOs ──

#[derive(Debug, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub repo_url: Option<String>,
    /// Legacy path field — prefer `git_root` for new projects.
    pub repo_path: Option<String>,
    pub default_branch: Option<String>,
    pub service_name: Option<String>,
    /// Package slug to assign (e.g. "software-dev", "researcher").
    /// Defaults to "software-dev" if omitted.
    pub package_slug: Option<String>,
    pub metadata: Option<serde_json::Value>,
    /// Tenant to assign this project to. Defaults to the user's first tenant.
    pub tenant_id: Option<Uuid>,
    /// Git topology mode: "monorepo" | "standalone" | "none". Defaults to "standalone".
    pub git_mode: Option<String>,
    /// Path to the git repository root (where .git lives), relative to PROJECTS_PATH.
    /// Required for all git modes. Falls back to repo_path if omitted.
    pub git_root: Option<String>,
    /// Subpath within git_root for the project directory. Only for monorepo mode.
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_playbook_id: Option<Option<Uuid>>,
    pub repo_url: Option<Option<String>>,
    /// Legacy path field — prefer `git_root` for new projects.
    pub repo_path: Option<Option<String>>,
    pub default_branch: Option<String>,
    pub service_name: Option<Option<String>>,
    /// Package slug to switch the project to (e.g. "researcher").
    pub package_slug: Option<String>,
    pub metadata: Option<serde_json::Value>,
    /// Git topology mode: "monorepo" | "standalone" | "none".
    pub git_mode: Option<String>,
    /// Path to the git repository root (where .git lives), relative to PROJECTS_PATH. Use None to clear.
    pub git_root: Option<Option<String>>,
    /// Subpath within git_root for the project directory (monorepo only). Use None to clear.
    pub project_root: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTask {
    pub title: String,
    pub kind: Option<String>,
    pub urgent: Option<bool>,
    pub context: Option<serde_json::Value>,
    pub required_capabilities: Option<Vec<String>>,
    pub playbook_id: Option<Uuid>,
    /// Optional FK to the decision that originated this task.
    pub decision_id: Option<Uuid>,
    /// Optional work item to link the new task to (inserts into task_work join table).
    pub work_id: Option<Uuid>,
    /// File paths this task intends to modify (for branch overlap detection).
    pub file_scope: Option<Vec<String>>,
    /// Optional parent task ID for plan decomposition (self-referencing FK).
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub urgent: Option<bool>,
    pub context: Option<serde_json::Value>,
    pub required_capabilities: Option<Vec<String>>,
    pub playbook_step: Option<i32>,
    /// Double-Option: None = don't change, Some(None) = clear, Some(Some(id)) = set.
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub playbook_id: Option<Option<Uuid>>,
    /// User-toggleable flag (bookmark).
    pub flagged: Option<bool>,
    /// File paths this task intends to modify (for branch overlap detection).
    pub file_scope: Option<Vec<String>>,
    /// Double-Option: None = don't change, Some(None) = clear, Some(Some(id)) = set parent.
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub parent_id: Option<Option<Uuid>>,
}

#[derive(Debug, Deserialize)]
pub struct TransitionTask {
    pub state: String,
    /// Optional playbook step index to set atomically with the state change.
    /// Used by the orchestra to advance/regress the pipeline in a single call.
    pub playbook_step: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimTask {
    pub agent_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgent {
    pub name: String,
    pub capabilities: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgent {
    pub name: Option<String>,
    pub status: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskUpdate {
    pub agent_id: Option<Uuid>,
    pub kind: Option<String>,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskComment {
    pub agent_id: Option<Uuid>,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AddDependency {
    pub depends_on: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub status: Option<String>,
}

// ── Domain Models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Work {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub work_type: String,
    pub priority: i32,
    pub parent_work_id: Option<Uuid>,
    pub auto_status: bool,
    pub intent_type: Option<String>,
    pub success_criteria: serde_json::Value,
    pub metadata: serde_json::Value,
    pub sort_order: i32,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskWork {
    pub task_id: Uuid,
    pub work_id: Uuid,
    pub position: i32,
}

// ── Work Comments ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkComment {
    pub id: Uuid,
    pub work_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Knowledge {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Embedding vector for semantic similarity search.
    /// Hidden from JSON responses to keep payloads small.
    #[serde(skip)]
    pub embedding: Option<Vec<f64>>,
}

/// A single alternative considered in a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionAlternative {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pros: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cons: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Decision {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub status: String,
    pub context: String,
    pub decision: Option<String>,
    pub rationale: Option<String>,
    #[sqlx(json)]
    pub alternatives: Vec<DecisionAlternative>,
    pub consequences: Option<String>,
    pub superseded_by: Option<Uuid>,
    pub tags: Vec<String>,
    pub decided_by: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Observation {
    pub id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub description: Option<String>,
    pub severity: String,
    pub status: String,
    pub evidence: serde_json::Value,
    pub resolved_task_id: Option<Uuid>,
    pub source: Option<String>,
    pub source_task_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Playbook {
    pub id: Uuid,
    /// Tenant that owns this playbook. NULL means shared / visible to all tenants.
    pub tenant_id: Option<Uuid>,
    pub title: String,
    pub trigger_description: Option<String>,
    pub steps: serde_json::Value,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    /// State a task enters when created with this playbook.
    /// "ready"   — auto-queue for agents immediately (default).
    /// "backlog" — stay in backlog until manually promoted to ready.
    pub initial_state: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Monotonically increasing version number, incremented on each update.
    pub version: i32,
    /// If this playbook was forked from another, the source playbook's ID.
    pub parent_id: Option<Uuid>,
    /// The version of the parent playbook at the time this fork was created.
    pub parent_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Event {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub source: String,
    pub title: String,
    pub description: Option<String>,
    pub severity: String,
    pub metadata: serde_json::Value,
    pub related_task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// ── New Request DTOs ──

#[derive(Debug, Deserialize)]
pub struct CreateWork {
    pub title: String,
    pub description: Option<String>,
    pub work_type: Option<String>,
    pub priority: Option<i32>,
    pub parent_work_id: Option<Uuid>,
    pub auto_status: Option<bool>,
    pub intent_type: Option<String>,
    pub success_criteria: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWork {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub work_type: Option<String>,
    pub priority: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub parent_work_id: Option<Option<Uuid>>,
    pub auto_status: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub intent_type: Option<Option<String>>,
    pub success_criteria: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderWorks {
    pub work_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct LinkTaskWork {
    pub task_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct BulkLinkTasks {
    pub task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkComment {
    pub agent_id: Option<Uuid>,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderWorkTasks {
    /// Ordered list of task IDs — position in the array becomes the order within the work item.
    pub task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateKnowledge {
    pub title: String,
    pub category: Option<String>,
    pub content: String,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateKnowledge {
    pub title: Option<String>,
    pub category: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDecision {
    pub title: String,
    pub context: String,
    pub decision: Option<String>,
    pub rationale: Option<String>,
    pub alternatives: Option<Vec<DecisionAlternative>>,
    pub consequences: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDecision {
    pub title: Option<String>,
    pub status: Option<String>,
    pub context: Option<String>,
    pub decision: Option<String>,
    pub rationale: Option<String>,
    pub alternatives: Option<Vec<DecisionAlternative>>,
    pub consequences: Option<String>,
    pub superseded_by: Option<Uuid>,
    pub decided_by: Option<Uuid>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateObservation {
    pub agent_id: Option<Uuid>,
    pub kind: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub severity: Option<String>,
    pub source: Option<String>,
    pub source_task_id: Option<Uuid>,
    pub evidence: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateObservation {
    pub title: Option<String>,
    pub description: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub evidence: Option<serde_json::Value>,
    pub resolved_task_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlaybook {
    pub title: String,
    pub trigger_description: Option<String>,
    pub steps: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    /// "ready" (default) or "backlog".
    pub initial_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlaybook {
    pub title: Option<String>,
    pub trigger_description: Option<String>,
    pub steps: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
    /// "ready" or "backlog".
    pub initial_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEvent {
    pub kind: String,
    pub source: String,
    pub title: String,
    pub description: Option<String>,
    pub severity: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub related_task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Default)]
pub struct WorkFilters {
    pub status: Option<String>,
    /// Comma-separated statuses to exclude (e.g. "achieved,paused,abandoned").
    pub status_not: Option<String>,
    pub work_type: Option<String>,
    pub parent_work_id: Option<Uuid>,
    pub top_level: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct KnowledgeFilters {
    pub category: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DecisionFilters {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ObservationFilters {
    pub kind: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PlaybookFilters {
    pub tag: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct EventFilters {
    pub kind: Option<String>,
    pub severity: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WorkProgress {
    pub work_id: Uuid,
    pub total_tasks: i64,
    pub done_tasks: i64,
}

#[derive(Debug, Serialize)]
pub struct WorkStats {
    pub work_id: Uuid,
    pub backlog_count: i64,
    pub ready_count: i64,
    pub working_count: i64,
    pub done_count: i64,
    pub cancelled_count: i64,
    pub total_count: i64,
    pub kind_breakdown: serde_json::Value,
    pub total_cost_usd: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub blocked_count: i64,
    pub avg_completion_hours: Option<f64>,
    pub oldest_open_task_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct PromoteObservation {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub urgent: Option<bool>,
    pub playbook_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupObservationsResult {
    pub deleted_dismissed: i64,
    pub deleted_acknowledged: i64,
    pub deleted_acted_on: i64,
    pub deleted_resolved: i64,
    pub deleted_duplicates: i64,
    pub deleted_old: i64,
    pub total_deleted: i64,
}

// ── Integration Models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Integration {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub kind: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub auth_type: String,
    pub credentials: serde_json::Value,
    pub config: serde_json::Value,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentIntegration {
    pub agent_id: Uuid,
    pub integration_id: Uuid,
    pub permissions: Vec<String>,
    pub role_id: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
    pub granted_by: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIntegration {
    pub name: String,
    pub kind: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub auth_type: Option<String>,
    pub credentials: Option<serde_json::Value>,
    pub config: Option<serde_json::Value>,
    pub capabilities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIntegration {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub base_url: Option<String>,
    pub auth_type: Option<String>,
    pub credentials: Option<serde_json::Value>,
    pub config: Option<serde_json::Value>,
    pub capabilities: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GrantAccess {
    pub agent_id: Uuid,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct IntegrationFilters {
    pub kind: Option<String>,
    pub provider: Option<String>,
    pub enabled: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Roles & Membership ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Role {
    pub id: Uuid,
    /// Tenant that owns this role.
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub authorities: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub knowledge_scope: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Membership {
    pub id: Uuid,
    /// Tenant this membership belongs to.
    pub tenant_id: Uuid,
    pub agent_id: Uuid,
    pub role_id: Uuid,
    pub status: String,
    pub config: serde_json::Value,
    pub joined_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRole {
    pub name: String,
    pub description: Option<String>,
    pub authorities: Option<Vec<String>>,
    pub required_capabilities: Option<Vec<String>>,
    pub knowledge_scope: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRole {
    pub name: Option<String>,
    pub description: Option<String>,
    pub authorities: Option<Vec<String>>,
    pub required_capabilities: Option<Vec<String>>,
    pub knowledge_scope: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMembership {
    pub agent_id: Uuid,
    pub role_id: Uuid,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMembership {
    pub status: Option<String>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct DelegateTask {
    pub agent_id: Uuid,
    pub role_id: Option<Uuid>,
}

// ── Bulk Actions ──

#[derive(Debug, Deserialize)]
pub struct BulkTransition {
    pub task_ids: Vec<Uuid>,
    pub state: String,
    /// Optional playbook step index to set atomically with the state change.
    pub playbook_step: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct BulkDelegate {
    pub task_ids: Vec<Uuid>,
    pub agent_id: Uuid,
    pub role_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct BulkDelete {
    pub task_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BulkResult {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<BulkFailure>,
}

#[derive(Debug, Serialize)]
pub struct BulkFailure {
    pub task_id: Uuid,
    pub error: String,
}

// ── Audit Log ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub actor_agent_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub summary: String,
    pub before_state: Option<serde_json::Value>,
    pub after_state: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AuditFilters {
    pub action: Option<String>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Agent Context (single-call operating context) ──

#[derive(Debug, Serialize)]
pub struct AgentContext {
    pub agent: Agent,
    pub membership: Membership,
    pub role: Role,
    pub project: Project,
    pub knowledge: Vec<Knowledge>,
    pub decisions: Vec<Decision>,
    pub integrations: Vec<Integration>,
    pub ready_tasks: Vec<Task>,
    pub my_tasks: Vec<Task>,
    pub open_observations: Vec<Observation>,
    pub recent_events: Vec<Event>,
    pub playbooks: Vec<Playbook>,
}

// ── File Locks ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileLock {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub path_glob: String,
    pub locked_by: Uuid,
    pub locked_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AcquireLocks {
    pub task_id: Uuid,
    pub paths: Vec<String>,
}

// ── Pagination ──

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub has_more: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct Pagination {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Webhook Models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Webhook {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,
    pub enabled: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub delivered_at: DateTime<Utc>,
    pub success: bool,
    pub attempt_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebhookDeadLetter {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub last_response_status: Option<i32>,
    pub last_response_body: Option<String>,
    pub attempts: i32,
    pub failed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhook {
    pub name: String,
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhook {
    pub name: Option<String>,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub events: Option<Vec<String>>,
    pub enabled: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

// ── Search ──

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub entity_types: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchResult {
    pub entity_type: String,
    pub id: Uuid,
    pub title: String,
    pub snippet: Option<String>,
    pub relevance: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: i64,
    pub query: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct TaskFilters {
    pub state: Option<String>,
    pub kind: Option<String>,
    pub agent_id: Option<Uuid>,
    pub ready_only: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Exclude done/cancelled tasks completed before this timestamp
    pub hide_done_before: Option<DateTime<Utc>>,
    /// Filter tasks linked to a specific decision
    pub decision_id: Option<Uuid>,
    /// Filter tasks linked to a specific work item
    pub work_id: Option<Uuid>,
    /// When true, return only tasks not linked to any work item
    pub unlinked: Option<bool>,
    /// Filter tasks by parent task ID (exact match)
    pub parent_id: Option<Uuid>,
    /// When true, return only top-level tasks (parent_id IS NULL)
    pub root_only: Option<bool>,
}

// ── Metrics ──

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    /// Number of days to look back (default 30)
    pub days: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ProjectMetrics {
    pub project_id: Uuid,
    pub range_days: i32,
    pub task_summary: TaskSummary,
    pub tasks_per_day: Vec<DayCount>,
    pub avg_time_in_state_hours: Vec<StateAvg>,
    pub agent_breakdown: Vec<AgentMetrics>,
    pub playbook_completion: Vec<PlaybookMetrics>,
    /// Aggregated cost across all tasks in the project within the range.
    pub cost_summary: CostSummary,
    /// Per-task cost breakdown for tasks with non-zero spend.
    pub task_costs: Vec<TaskCostRow>,
    /// Token usage per day for time-series graphing.
    pub tokens_per_day: Vec<TokenDayCount>,
}

/// Aggregated LLM token usage and cost for a project.
#[derive(Debug, Serialize)]
pub struct CostSummary {
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_usd: f64,
}

/// Per-task cost row returned in the metrics response.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TaskCostRow {
    pub task_id: Uuid,
    pub task_number: i64,
    pub title: String,
    pub state: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

/// Request body for recording LLM usage after a completed step.
#[derive(Debug, Deserialize)]
pub struct TaskCostUpdate {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct TaskSummary {
    pub total: i64,
    pub done: i64,
    pub cancelled: i64,
    pub in_progress: i64,
    pub ready: i64,
    pub backlog: i64,
    pub human_review: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DayCount {
    pub day: chrono::NaiveDate,
    pub count: i64,
}

/// Token usage per day for time-series graphing.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TokenDayCount {
    pub day: chrono::NaiveDate,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StateAvg {
    pub state: String,
    pub avg_hours: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AgentMetrics {
    pub agent_id: Uuid,
    pub agent_name: String,
    pub tasks_completed: i64,
    pub tasks_in_progress: i64,
    pub avg_completion_hours: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PlaybookMetrics {
    pub playbook_id: Uuid,
    pub playbook_title: String,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub completion_rate: f64,
}

// ── Verification ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Verification {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub detail: Option<String>,
    pub evidence: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVerification {
    pub task_id: Option<Uuid>,
    pub kind: String,
    pub status: Option<String>,
    pub title: String,
    pub detail: Option<String>,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVerification {
    pub status: Option<String>,
    pub detail: Option<String>,
    pub evidence: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct VerificationFilters {
    pub task_id: Option<Uuid>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Changed Files ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChangedFile {
    pub id: Uuid,
    pub task_id: Uuid,
    pub path: String,
    pub change_type: String,
    pub diff: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Summary version without the diff content (for list responses)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChangedFileSummary {
    pub id: Uuid,
    pub task_id: Uuid,
    pub path: String,
    pub change_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChangedFile {
    pub path: String,
    pub change_type: String,
    pub diff: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChangedFiles {
    pub files: Vec<CreateChangedFile>,
}

// ── Tenant Models ──

pub const TENANT_ROLES: &[&str] = &["owner", "admin", "member"];
pub const ENCRYPTION_MODES: &[&str] = &["none", "login_derived", "passphrase"];

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub encryption_mode: String,
    pub key_salt: Option<String>,
    pub theme_preference: String,
    pub accent_color: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TenantMember {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WrappedKey {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub key_type: String,
    pub wrapped_dek: String,
    pub kdf_salt: String,
    pub kdf_params: Option<serde_json::Value>,
    pub key_version: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenant {
    pub name: String,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenant {
    pub name: Option<String>,
    pub encryption_mode: Option<String>,
    pub key_salt: Option<String>,
    pub theme_preference: Option<String>,
    pub accent_color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTenantMember {
    pub user_id: Uuid,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantMember {
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWrappedKey {
    pub key_type: String,
    pub wrapped_dek: String,
    pub kdf_salt: String,
    pub kdf_params: Option<serde_json::Value>,
    pub key_version: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct TenantFilters {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Reports ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Report {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub kind: String,
    pub prompt: String,
    pub status: String,
    pub result: Option<String>,
    pub task_id: Option<Uuid>,
    pub created_by: Uuid,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReport {
    pub title: String,
    pub kind: String,
    pub prompt: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReport {
    pub title: Option<String>,
    pub status: Option<String>,
    pub result: Option<String>,
    pub task_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportFilters {
    pub status: Option<String>,
    pub kind: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Request body for `POST /{project_id}/reports/{id}/complete`.
#[derive(Debug, Deserialize)]
pub struct CompleteReport {
    pub result: String,
    pub status: Option<String>,
}

// ── Task Logs ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskLog {
    pub id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub step_name: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Summary view returned by list endpoints (excludes large `content` field).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskLogSummary {
    pub id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub step_name: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskLog {
    pub task_id: Uuid,
    pub step_name: Option<String>,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TaskLogFilters {
    pub task_id: Option<Uuid>,
    pub step_name: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Step Templates ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StepTemplate {
    pub id: Uuid,
    /// Tenant that owns this template. NULL means global / visible to all tenants.
    pub tenant_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
    pub budget: Option<f64>,
    pub allowed_tools: Option<String>,
    pub context_level: Option<String>,
    pub on_complete: Option<String>,
    pub retriable: Option<bool>,
    pub max_cycles: Option<i32>,
    pub timeout_minutes: Option<i32>,
    pub mcp_servers: Option<serde_json::Value>,
    pub agents: Option<serde_json::Value>,
    pub agent: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub env: Option<serde_json::Value>,
    pub vars: Option<serde_json::Value>,
    /// AI provider for this step (e.g. "anthropic", "openai", "ollama"). NULL defaults to "anthropic".
    pub provider: Option<String>,
    /// Override the default API endpoint for the chosen provider.
    pub base_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStepTemplate {
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
    pub budget: Option<f64>,
    pub allowed_tools: Option<String>,
    pub context_level: Option<String>,
    pub on_complete: Option<String>,
    pub retriable: Option<bool>,
    pub max_cycles: Option<i32>,
    pub timeout_minutes: Option<i32>,
    pub mcp_servers: Option<serde_json::Value>,
    pub agents: Option<serde_json::Value>,
    pub agent: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub env: Option<serde_json::Value>,
    pub vars: Option<serde_json::Value>,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateStepTemplate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub model: Option<String>,
    pub budget: Option<f64>,
    pub allowed_tools: Option<String>,
    pub context_level: Option<String>,
    pub on_complete: Option<String>,
    pub retriable: Option<bool>,
    pub max_cycles: Option<i32>,
    pub timeout_minutes: Option<i32>,
    pub mcp_servers: Option<serde_json::Value>,
    pub agents: Option<serde_json::Value>,
    pub agent: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub env: Option<serde_json::Value>,
    pub vars: Option<serde_json::Value>,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
pub struct StepTemplateFilters {
    pub name: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Event Observation Rule Models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventObservationRule {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub event_kind: Option<String>,
    pub event_source: Option<String>,
    pub severity_gte: Option<String>,
    pub observation_kind: String,
    pub observation_severity: String,
    pub title_template: String,
    pub description_template: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEventObservationRule {
    pub name: String,
    pub enabled: Option<bool>,
    pub event_kind: Option<String>,
    pub event_source: Option<String>,
    pub severity_gte: Option<String>,
    pub observation_kind: Option<String>,
    pub observation_severity: Option<String>,
    pub title_template: String,
    pub description_template: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEventObservationRule {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub event_kind: Option<String>,
    pub event_source: Option<String>,
    pub severity_gte: Option<String>,
    pub observation_kind: Option<String>,
    pub observation_severity: Option<String>,
    pub title_template: Option<String>,
    pub description_template: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct EventObservationRuleFilters {
    pub enabled: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Related Items ──

/// A single related item (knowledge, decision, or observation) with a relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedItem {
    pub entity_type: String,
    pub id: Uuid,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    pub relevance_score: f64,
    pub reason: String,
}

/// Grouped related items by entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedItems {
    pub knowledge: Vec<RelatedItem>,
    pub decisions: Vec<RelatedItem>,
    pub observations: Vec<RelatedItem>,
}

/// Used by the stale-task detector to carry task+agent info across backends.
#[derive(Debug)]
pub struct StaleTaskInfo {
    pub task_id: Uuid,
    pub task_title: String,
    pub task_state: String,
    pub project_id: Uuid,
    pub agent_id: Uuid,
    pub agent_name: String,
    pub agent_last_seen_at: Option<DateTime<Utc>>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub auto_release: bool,
}

// ── Provider Config Models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderConfig {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Option<Uuid>,
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ProviderConfigFilters {
    pub provider: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Merged provider config produced by the resolution function.
/// Project-level overrides global, with api_key falling back to global if absent.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedProviderConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    /// Which config contributed the api_key: "project", "global", or null.
    pub api_key_source: Option<String>,
}

// ── Forgejo CI ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ForgejoIntegration {
    pub id: Uuid,
    pub project_id: Uuid,
    pub base_url: String,
    pub token: Option<String>,
    pub webhook_secret: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CiRun {
    pub id: Uuid,
    pub project_id: Uuid,
    pub external_id: i64,
    pub provider: String,
    pub workflow_name: String,
    pub status: String,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub triggered_by: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CiJob {
    pub id: Uuid,
    pub ci_run_id: Uuid,
    pub name: String,
    pub status: String,
    pub runner: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CiStep {
    pub id: Uuid,
    pub ci_job_id: Uuid,
    pub name: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

// ── CI REST API DTOs ──

#[derive(Debug, Deserialize)]
pub struct CiRunFilters {
    pub branch: Option<String>,
    pub status: Option<String>,
    pub workflow_name: Option<String>,
    pub provider: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CiRunWithJobs {
    #[serde(flatten)]
    pub run: CiRun,
    pub jobs: Vec<CiJob>,
}

#[derive(Debug, Serialize)]
pub struct CiJobWithSteps {
    #[serde(flatten)]
    pub job: CiJob,
    pub steps: Vec<CiStep>,
}

#[derive(Debug, Deserialize)]
pub struct CreateForgejoIntegration {
    pub base_url: String,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ForgejoIntegrationResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub base_url: String,
    pub webhook_url: String,
    pub webhook_secret: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── GitHub CI ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GitHubIntegration {
    pub id: Uuid,
    pub project_id: Uuid,
    pub base_url: String,
    pub token: Option<String>,
    pub webhook_secret: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGitHubIntegration {
    pub base_url: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitHubIntegrationResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub base_url: String,
    pub webhook_url: String,
    pub webhook_secret: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
