use std::collections::HashMap;

use crate::client::{
    Agent, AuditEntry, BranchInfo, ChangedFile, ChatMessage, Decision, GitTaskStatus, Integration,
    IntegrationAccess, KnowledgeEntry, LogEntry, MainPushStatus, Member, Observation, Playbook,
    Project, ProjectEvent, ProjectMetrics, Report, Role, SearchResult, StepTemplate, Task,
    TaskComment, TaskDependencies, TaskUpdate, TreeEntry, Verification, Webhook, WebhookDelivery,
    Work, WorkComment, WorkProgress, WorkStats,
};
use ratatui::widgets::ListState;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tasks,
    Agents,
    Knowledge,
    Decisions,
    Playbooks,
    Work,
    Observations,
    Team,
    Integrations,
    Audit,
    Logs,
    ProjectSettings,
    Verifications,
    Git,
    Search,
    Chat,
    Source,
    Dashboard,
    Reports,
    Events,
    Webhooks,
    StepTemplates,
}

/// View ordering matches navigation.json — core → operations → reference → tools → system.
pub const ALL_VIEWS: &[View] = &[
    // core
    View::Work,
    View::Tasks,
    View::Decisions,
    View::Dashboard,
    // operations
    View::Agents,
    View::Playbooks,
    View::Observations,
    View::Team,
    // reference
    View::Knowledge,
    View::Verifications,
    View::Reports,
    // tools
    View::Git,
    View::Source,
    View::Search,
    View::Chat,
    // system
    View::Logs,
    View::Audit,
    View::Events,
    View::Integrations,
    View::Webhooks,
    View::ProjectSettings,
    View::StepTemplates,
];

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Tasks => "Tasks",
            View::Agents => "Agents",
            View::Knowledge => "Knowledge",
            View::Decisions => "Decisions",
            View::Playbooks => "Playbooks",
            View::Work => "Work",
            View::Observations => "Observations",
            View::Team => "Team",
            View::Integrations => "Integrations",
            View::Audit => "Audit",
            View::Logs => "Logs",
            View::ProjectSettings => "Project Settings",
            View::Verifications => "Verifications",
            View::Git => "Git",
            View::Search => "Search",
            View::Chat => "Chat",
            View::Source => "Source",
            View::Dashboard => "Dashboard",
            View::Reports => "Reports",
            View::Events => "Events",
            View::Webhooks => "Webhooks",
            View::StepTemplates => "Step Templates",
        }
    }

    /// Keyboard shortcut — kept in sync with navigation.json.
    pub fn shortcut(self) -> &'static str {
        match self {
            View::Work => "1",
            View::Tasks => "2",
            View::Decisions => "3",
            View::Dashboard => "4",
            View::Agents => "5",
            View::Playbooks => "6",
            View::Observations => "7",
            View::Team => "8",
            View::Knowledge => "9",
            View::Verifications => "0",
            View::Reports => "R",
            View::Git => "G",
            View::Source => "B",
            View::Search => "F",
            View::Chat => "C",
            View::Logs => "`",
            View::Audit => "A",
            View::Events => "E",
            View::Integrations => "I",
            View::Webhooks => "W",
            View::ProjectSettings => "S",
            View::StepTemplates => "T",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    None,
    Transition,
    Reply,
    Comment,
    Search,
    WorkStatus,
    Promote,
    DependencyAdd,
    LogQuery,
    LogFilter,
    ObservationStatus,
    DecisionSupersede,
    IntegrationAccess,
    ViewPicker,
    ClaimTask,
    DelegateTask,
    BulkTransition,
    BulkDelete,
    VerificationStatus,
    VerificationKindFilter,
    VerificationStatusFilter,
    GlobalSearch,
    ChatInput,
    WorkComment,
    EventKindFilter,
    EventSeverityFilter,
    ReportDelete,
}

pub const TIME_RANGES: &[(&str, i64)] = &[
    ("15m", 15 * 60),
    ("1h", 3600),
    ("3h", 3 * 3600),
    ("6h", 6 * 3600),
    ("12h", 12 * 3600),
    ("24h", 24 * 3600),
    ("3d", 3 * 86400),
    ("7d", 7 * 86400),
];

pub const LOG_LIMITS: &[u32] = &[50, 100, 200, 500, 1000];

pub const LOG_DIRECTIONS: &[&str] = &["backward", "forward"];

pub const TASK_KINDS: &[&str] = &[
    "feature", "bug", "chore", "research", "spike", "refactor", "docs", "test",
];

pub const VERIFICATION_KINDS: &[&str] = &["test", "acceptance", "sign_off"];
pub const VERIFICATION_STATUSES: &[&str] = &["pass", "fail", "pending", "skipped"];

#[derive(Default)]
pub struct TaskForm {
    pub title: String,
    pub kind_index: usize,
    pub urgent: bool,
    pub spec: String,
    pub playbook_index: usize, // 0 = None, 1+ = playbook from list
    pub active_field: usize,   // 0=title, 1=kind, 2=urgent, 3=playbook, 4=spec
    pub cursor: usize,
}

pub struct VerificationForm {
    pub task_index: usize,   // Index into tasks list (for task_id picker)
    pub kind_index: usize,   // Index into VERIFICATION_KINDS
    pub status_index: usize, // Index into VERIFICATION_STATUSES
    pub title: String,
    pub detail: String,
    pub evidence: String,    // JSON string
    pub active_field: usize, // 0=task, 1=kind, 2=status, 3=title, 4=detail, 5=evidence
    pub cursor: usize,
    pub editing_id: Option<uuid::Uuid>, // None for create, Some for edit
}

impl Default for VerificationForm {
    fn default() -> Self {
        Self {
            task_index: 0,
            kind_index: 0,
            status_index: 0,
            title: String::new(),
            detail: String::new(),
            evidence: String::new(),
            active_field: 3, // Start on title field
            cursor: 0,
            editing_id: None,
        }
    }
}

pub const ON_COMPLETE_OPTIONS: &[&str] = &["next", "done", "human_review"];
pub const STEP_MODEL_OPTIONS: &[&str] = &["", "sonnet", "opus", "haiku"];

#[derive(Clone, Default)]
pub struct PlaybookStepForm {
    pub name: String,
    pub description: String,
    pub on_complete_index: usize, // index into ON_COMPLETE_OPTIONS
    pub timeout_minutes: String,
    pub model_index: usize, // index into STEP_MODEL_OPTIONS (0 = default/none)
}

#[derive(Default)]
pub struct PlaybookForm {
    pub title: String,
    pub trigger: String,
    pub tags: String, // comma-separated
    pub steps: Vec<PlaybookStepForm>,
    pub active_field: usize, // 0=title, 1=trigger, 2=tags, 3=steps
    pub cursor: usize,
    pub editing_id: Option<uuid::Uuid>, // None for create, Some for edit
    // Step management state
    pub selected_step: usize,
    pub editing_step: bool, // true when editing step detail fields
    pub step_field: usize,  // 0=name, 1=desc, 2=on_complete, 3=timeout, 4=model
}

impl PlaybookForm {
    /// Parse steps from a serde_json::Value (JSON array) into PlaybookStepForm vec
    pub fn steps_from_json(steps: &serde_json::Value) -> Vec<PlaybookStepForm> {
        let Some(arr) = steps.as_array() else {
            return Vec::new();
        };
        let mut result: Vec<PlaybookStepForm> = arr
            .iter()
            .map(|step| {
                let name = step
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = step
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let on_complete = step
                    .get("on_complete")
                    .and_then(|v| v.as_str())
                    .unwrap_or("next");
                let on_complete_index = ON_COMPLETE_OPTIONS
                    .iter()
                    .position(|o| *o == on_complete)
                    .unwrap_or(0);
                let timeout = step
                    .get("timeout_minutes")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let model = step.get("model").and_then(|v| v.as_str()).unwrap_or("");
                let model_index = STEP_MODEL_OPTIONS
                    .iter()
                    .position(|o| *o == model)
                    .unwrap_or(0);
                PlaybookStepForm {
                    name,
                    description,
                    on_complete_index,
                    timeout_minutes: timeout,
                    model_index,
                }
            })
            .collect();
        // Sort by step order from JSON
        // (steps are already in order in the array, but let's be safe)
        result.sort_by_key(|_| 0); // preserve original order
        result
    }

    /// Convert steps back to JSON array
    pub fn steps_to_json(&self) -> serde_json::Value {
        let arr: Vec<serde_json::Value> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                // Normalize step name to snake_case (match web behaviour):
                // trim, lowercase, collapse whitespace to underscores.
                let raw = s.name.trim().to_lowercase();
                let normalized: String = raw.split_whitespace().collect::<Vec<_>>().join("_");
                let name = if normalized.is_empty() {
                    format!("step_{i}")
                } else {
                    normalized
                };
                let mut obj = serde_json::json!({
                    "step": i,
                    "name": name,
                    "description": s.description,
                    "on_complete": ON_COMPLETE_OPTIONS[s.on_complete_index],
                });
                if let Ok(mins) = s.timeout_minutes.parse::<i64>() {
                    obj["timeout_minutes"] = serde_json::json!(mins);
                }
                let model = STEP_MODEL_OPTIONS[s.model_index];
                if !model.is_empty() {
                    obj["model"] = serde_json::json!(model);
                }
                obj
            })
            .collect();
        serde_json::Value::Array(arr)
    }
}

/// Task edit form — used for editing existing task properties
pub struct TaskEditForm {
    pub task_id: uuid::Uuid,
    pub title: String,
    pub kind_index: usize,
    pub urgent: bool,
    pub spec: String,
    pub active_field: usize, // 0=title, 1=kind, 2=urgent, 3=spec
    pub cursor: usize,
}

/// Fields: 0=name, 1=description, 2=repo_url, 3=repo_path,
///         4=default_branch, 5=service_name, 6=playbook, 7=claude_md
pub const SETTINGS_FIELD_COUNT: usize = 8;

pub struct ProjectSettingsForm {
    pub name: String,
    pub description: String,
    pub repo_url: String,
    pub repo_path: String,
    pub default_branch: String,
    pub service_name: String,
    pub playbook_index: usize, // 0=None, 1+=playbook from list
    pub claude_md: String,
    pub active_field: usize,
    pub cursor: usize,
    pub dirty: bool, // track unsaved changes
}

impl ProjectSettingsForm {
    pub fn field_text(&self) -> &str {
        match self.active_field {
            0 => &self.name,
            1 => &self.description,
            2 => &self.repo_url,
            3 => &self.repo_path,
            4 => &self.default_branch,
            5 => &self.service_name,
            7 => &self.claude_md,
            _ => "",
        }
    }

    pub fn field_text_mut(&mut self) -> Option<&mut String> {
        match self.active_field {
            0 => Some(&mut self.name),
            1 => Some(&mut self.description),
            2 => Some(&mut self.repo_url),
            3 => Some(&mut self.repo_path),
            4 => Some(&mut self.default_branch),
            5 => Some(&mut self.service_name),
            7 => Some(&mut self.claude_md),
            _ => None,
        }
    }
}

pub const REPORT_KINDS: &[&str] = &[
    "security",
    "component",
    "architecture",
    "performance",
    "custom",
];

#[derive(Default)]
pub struct ReportForm {
    pub title: String,
    pub kind_index: usize, // index into REPORT_KINDS
    pub prompt: String,
    pub active_field: usize, // 0=title, 1=kind, 2=prompt
    pub cursor: usize,
}

pub const WEBHOOK_EVENT_TYPES: &[&str] = &[
    "task.created",
    "task.updated",
    "task.transitioned",
    "task.completed",
    "task.commented",
    "work.created",
    "work.updated",
    "decision.created",
    "decision.updated",
    "observation.created",
    "knowledge.created",
    "verification.created",
];

pub struct WebhookForm {
    pub url: String,
    pub secret: String,
    pub event_toggles: Vec<bool>, // parallel to WEBHOOK_EVENT_TYPES
    pub active_field: usize,      // 0=url, 1=secret, 2=events
    pub cursor: usize,
    pub event_selected: usize, // which event in the list is highlighted
}

impl Default for WebhookForm {
    fn default() -> Self {
        Self {
            url: String::new(),
            secret: String::new(),
            event_toggles: vec![false; WEBHOOK_EVENT_TYPES.len()],
            active_field: 0,
            cursor: 0,
            event_selected: 0,
        }
    }
}

pub const EVENT_KINDS: &[&str] = &[
    "ci", "deploy", "error", "merge", "release", "alert", "custom",
];
pub const EVENT_SEVERITIES: &[&str] = &["info", "warning", "error", "critical"];

pub struct EventForm {
    pub title: String,
    pub kind_index: usize,     // index into EVENT_KINDS
    pub severity_index: usize, // index into EVENT_SEVERITIES
    pub description: String,
    pub active_field: usize, // 0=title, 1=kind, 2=severity, 3=description
    pub cursor: usize,
}

impl Default for EventForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            kind_index: 6,     // default to "custom"
            severity_index: 0, // default to "info"
            description: String::new(),
            active_field: 0,
            cursor: 0,
        }
    }
}

pub const WORK_STATUSES: &[&str] = &["active", "achieved", "paused", "abandoned"];
pub const WORK_TYPES: &[&str] = &["epic", "feature", "milestone", "sprint", "initiative"];

pub struct WorkForm {
    pub title: String,
    pub description: String,
    pub success_criteria: String,
    pub status_index: usize,    // index into WORK_STATUSES
    pub work_type_index: usize, // index into WORK_TYPES
    pub priority: String,       // numeric string
    pub auto_status: bool,
    pub active_field: usize, // 0=title, 1=desc, 2=criteria, 3=status, 4=type, 5=priority, 6=auto_status
    pub cursor: usize,
    pub editing_id: Option<Uuid>, // None for create, Some for edit
}

impl Default for WorkForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            success_criteria: String::new(),
            status_index: 0,
            work_type_index: 0,
            priority: "0".to_string(),
            auto_status: false,
            active_field: 0,
            cursor: 0,
            editing_id: None,
        }
    }
}

pub const OBSERVATION_KINDS: &[&str] = &[
    "insight",
    "risk",
    "smell",
    "improvement",
    "opportunity",
    "inconsistency",
];
pub const OBSERVATION_SEVERITIES: &[&str] = &["info", "low", "medium", "high", "critical"];

pub struct ObservationForm {
    pub title: String,
    pub description: String,
    pub kind_index: usize,     // index into OBSERVATION_KINDS
    pub severity_index: usize, // index into OBSERVATION_SEVERITIES
    pub active_field: usize,   // 0=title, 1=kind, 2=severity, 3=description
    pub cursor: usize,
}

impl Default for ObservationForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            kind_index: 0,
            severity_index: 2, // default to "medium"
            active_field: 0,
            cursor: 0,
        }
    }
}

#[derive(Default)]
pub struct DecisionForm {
    pub title: String,
    pub context: String,
    pub decision: String,
    pub rationale: String,
    pub alternatives: String, // free-text, one per line
    pub active_field: usize,  // 0=title, 1=context, 2=decision, 3=rationale, 4=alternatives
    pub cursor: usize,
}

pub const KNOWLEDGE_CATEGORIES: &[&str] = &[
    "architecture",
    "convention",
    "pattern",
    "anti_pattern",
    "setup",
    "general",
];

#[derive(Default)]
pub struct KnowledgeForm {
    pub title: String,
    pub category_index: usize, // index into KNOWLEDGE_CATEGORIES
    pub content: String,
    pub tags: String,        // comma-separated
    pub active_field: usize, // 0=title, 1=category, 2=content, 3=tags
    pub cursor: usize,
    pub editing_id: Option<Uuid>, // None for create, Some for edit
}

pub const INTEGRATION_KINDS: &[&str] = &["ci", "monitoring", "logging", "vcs", "chat", "custom"];
pub const INTEGRATION_AUTH_TYPES: &[&str] = &["none", "token", "basic", "oauth2"];

#[derive(Default)]
pub struct IntegrationForm {
    pub name: String,
    pub kind_index: usize, // index into INTEGRATION_KINDS
    pub provider: String,
    pub base_url: String,
    pub auth_type_index: usize, // index into INTEGRATION_AUTH_TYPES
    pub active_field: usize,    // 0=name, 1=kind, 2=provider, 3=base_url, 4=auth_type
    pub cursor: usize,
}

pub struct App {
    pub show_help: bool,
    pub task_form: Option<TaskForm>,
    pub task_edit_form: Option<TaskEditForm>,
    pub playbook_form: Option<PlaybookForm>,
    pub settings_form: Option<ProjectSettingsForm>,
    pub verification_form: Option<VerificationForm>,
    pub work_form: Option<WorkForm>,
    pub observation_form: Option<ObservationForm>,
    pub decision_form: Option<DecisionForm>,
    pub knowledge_form: Option<KnowledgeForm>,
    pub integration_form: Option<IntegrationForm>,
    pub event_form: Option<EventForm>,
    pub webhook_form: Option<WebhookForm>,
    pub report_form: Option<ReportForm>,
    pub connected: bool,
    pub view: View,
    pub modal: Modal,
    pub focus: usize, // 0=left, 1=right

    // Data
    pub projects: Vec<Project>,
    pub current_project: Option<Uuid>,
    pub tasks: Vec<Task>,
    pub task_updates: Vec<TaskUpdate>,
    pub task_comments: Vec<TaskComment>,
    pub task_dependencies: TaskDependencies,
    pub agents: Vec<Agent>,
    pub knowledge: Vec<KnowledgeEntry>,
    pub decisions: Vec<Decision>,
    pub playbooks: Vec<Playbook>,
    pub work_items: Vec<Work>,
    pub work_progress: Option<WorkProgress>,
    pub work_progress_map: HashMap<Uuid, WorkProgress>,
    pub work_stats: Option<WorkStats>,
    pub work_children: Vec<Work>,
    pub observations: Vec<Observation>,
    pub roles: Vec<Role>,
    pub members: Vec<Member>,
    pub integrations: Vec<Integration>,
    pub integration_access: Vec<IntegrationAccess>,
    pub audit_log: Vec<AuditEntry>,
    pub verifications: Vec<Verification>,
    pub events: Vec<ProjectEvent>,
    pub event_kind_filter: Option<String>,
    pub event_severity_filter: Option<String>,
    pub webhooks: Vec<Webhook>,
    pub webhook_deliveries: Vec<WebhookDelivery>,
    pub webhook_test_result: Option<String>,
    pub reports: Vec<Report>,
    pub selected_report: Option<usize>,
    // Dashboard
    pub dashboard_metrics: Option<ProjectMetrics>,
    pub dashboard_events: Vec<ProjectEvent>,

    // Work comments
    pub work_comments: Vec<WorkComment>,

    // Step templates
    pub step_templates: Vec<StepTemplate>,

    // Agent tasks (queue view)
    pub agent_tasks: Vec<Task>,

    // Work tasks
    pub work_tasks: Vec<Task>,

    // Verification filters
    pub verification_kind_filter: Option<String>,
    pub verification_status_filter: Option<String>,

    // Logs view data
    pub log_entries: Vec<LogEntry>,
    pub log_labels: Vec<String>,
    pub log_query: String,
    pub log_filter: String,
    pub log_time_range_idx: usize, // index into TIME_RANGES
    pub log_limit_idx: usize,      // index into LOG_LIMITS
    pub log_direction_idx: usize,  // index into LOG_DIRECTIONS
    pub log_loading: bool,
    pub log_scroll: u16,

    // Selection indices
    pub selected_task: Option<usize>,
    pub task_list_state: ListState,
    pub selected_agent: Option<usize>,
    pub selected_knowledge: Option<usize>,
    pub selected_decision: Option<usize>,
    pub selected_playbook: Option<usize>,
    pub selected_work: Option<usize>,
    pub work_list_state: ListState,
    pub selected_observation: Option<usize>,
    pub selected_role: Option<usize>,
    pub selected_member: Option<usize>,
    pub selected_integration: Option<usize>,
    pub selected_audit: Option<usize>,
    pub selected_verification: Option<usize>,
    pub selected_event: Option<usize>,
    pub selected_webhook: Option<usize>,

    // Team view focus: 0=roles, 1=members, 2=detail
    pub team_focus: usize,

    // Search filter
    pub search_query: String,

    // Detail panel scroll offset (vertical)
    pub detail_scroll: u16,

    // Modal state
    pub transition_options: Vec<String>,
    pub transition_selected: usize,
    pub input_text: String,
    pub input_cursor: usize,

    // Git integration
    pub git_task_status: Option<GitTaskStatus>,
    pub changed_files: Vec<ChangedFile>,
    pub show_changed_files: bool,

    // Bulk selection
    pub bulk_selected: std::collections::HashSet<uuid::Uuid>,
    pub bulk_mode: bool,

    // Task hierarchy
    pub show_hierarchy: bool,
    pub subtasks: Vec<Task>,

    // Git view
    pub branches: Vec<BranchInfo>,
    pub current_branch: String,
    pub main_push_status: Option<MainPushStatus>,
    pub selected_branch: Option<usize>,
    pub git_action_result: Option<String>,

    // Search view
    pub search_results: Vec<SearchResult>,
    pub search_total: i64,
    pub search_executed_query: String,
    pub selected_search_result: Option<usize>,

    // Chat view
    pub chat_messages: Vec<ChatMessage>,
    pub chat_input: String,
    pub chat_streaming: bool,
    pub chat_scroll: u16,

    // Source browser
    pub source_entries: Vec<TreeEntry>,
    pub source_current_path: String,
    pub source_selected: Option<usize>,
    pub source_blob_content: Option<String>,
    pub source_blob_path: Option<String>,
    pub source_blob_scroll: u16,

    // Error flash
    pub last_error: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            connected: false,
            show_help: false,
            task_form: None,
            task_edit_form: None,
            playbook_form: None,
            settings_form: None,
            verification_form: None,
            work_form: None,
            observation_form: None,
            decision_form: None,
            knowledge_form: None,
            integration_form: None,
            event_form: None,
            webhook_form: None,
            report_form: None,
            view: View::Work,
            modal: Modal::None,
            focus: 0,
            projects: vec![],
            current_project: None,
            tasks: vec![],
            task_updates: vec![],
            task_comments: vec![],
            task_dependencies: TaskDependencies {
                depends_on: vec![],
                blocks: vec![],
            },
            agents: vec![],
            knowledge: vec![],
            decisions: vec![],
            playbooks: vec![],
            work_items: vec![],
            work_progress: None,
            work_progress_map: HashMap::new(),
            work_stats: None,
            work_children: vec![],
            observations: vec![],
            roles: vec![],
            members: vec![],
            integrations: vec![],
            integration_access: vec![],
            audit_log: vec![],
            verifications: vec![],
            events: vec![],
            event_kind_filter: None,
            event_severity_filter: None,
            webhooks: vec![],
            webhook_deliveries: vec![],
            webhook_test_result: None,
            reports: vec![],
            selected_report: None,
            dashboard_metrics: None,
            dashboard_events: vec![],
            work_comments: vec![],
            step_templates: vec![],
            agent_tasks: vec![],
            work_tasks: vec![],
            verification_kind_filter: None,
            verification_status_filter: None,
            log_entries: vec![],
            log_labels: vec![],
            log_query: String::from("{app=~\".+\"}"),
            log_filter: String::new(),
            log_time_range_idx: 1, // 1h default
            log_limit_idx: 1,      // 100 default
            log_direction_idx: 0,  // backward default
            log_loading: false,
            log_scroll: 0,
            selected_task: None,
            task_list_state: ListState::default(),
            selected_agent: None,
            selected_knowledge: None,
            selected_decision: None,
            selected_playbook: None,
            selected_work: None,
            work_list_state: ListState::default(),
            selected_observation: None,
            selected_role: None,
            selected_member: None,
            selected_integration: None,
            selected_audit: None,
            selected_verification: None,
            selected_event: None,
            selected_webhook: None,
            team_focus: 0,
            search_query: String::new(),
            detail_scroll: 0,
            transition_options: vec![],
            transition_selected: 0,
            input_text: String::new(),
            input_cursor: 0,
            git_task_status: None,
            changed_files: vec![],
            show_changed_files: false,
            bulk_selected: std::collections::HashSet::new(),
            bulk_mode: false,
            show_hierarchy: false,
            subtasks: vec![],
            branches: vec![],
            current_branch: String::new(),
            main_push_status: None,
            selected_branch: None,
            git_action_result: None,
            search_results: vec![],
            search_total: 0,
            search_executed_query: String::new(),
            selected_search_result: None,
            chat_messages: vec![],
            chat_input: String::new(),
            chat_streaming: false,
            chat_scroll: 0,
            source_entries: vec![],
            source_current_path: String::new(),
            source_selected: None,
            source_blob_content: None,
            source_blob_path: None,
            source_blob_scroll: 0,
            last_error: None,
        }
    }

    pub fn list_len(&self) -> usize {
        match self.view {
            View::Tasks => self.tasks.len(),
            View::Agents => self.agents.len(),
            View::Knowledge => self.knowledge.len(),
            View::Decisions => self.decisions.len(),
            View::Playbooks => self.playbooks.len(),
            View::Work => self.work_items.len(),
            View::Observations => self.observations.len(),
            View::Team => self.roles.len(),
            View::Integrations => self.integrations.len(),
            View::Audit => self.audit_log.len(),
            View::Logs => self.log_entries.len(),
            View::ProjectSettings => 0, // No list in settings view
            View::Verifications => self.filtered_verifications().len(),
            View::Git => self.branches.len(),
            View::Search => self.search_results.len(),
            View::Chat => self.chat_messages.len(),
            View::Source => self.source_entries.len(),
            View::Dashboard | View::StepTemplates => 0,
            View::Reports => self.reports.len(),
            View::Webhooks => self.webhooks.len(),
            View::Events => self.filtered_events().len(),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        match self.view {
            View::Tasks => self.selected_task,
            View::Agents => self.selected_agent,
            View::Knowledge => self.selected_knowledge,
            View::Decisions => self.selected_decision,
            View::Playbooks => self.selected_playbook,
            View::Work => self.selected_work,
            View::Observations => self.selected_observation,
            View::Team => self.selected_role,
            View::Integrations => self.selected_integration,
            View::Audit => self.selected_audit,
            View::Logs => None, // Logs use scroll, not selection
            View::ProjectSettings => None,
            View::Verifications => self.selected_verification,
            View::Git => self.selected_branch,
            View::Search => self.selected_search_result,
            View::Chat => None,
            View::Source => self.source_selected,
            View::Dashboard | View::StepTemplates => None,
            View::Reports => self.selected_report,
            View::Webhooks => self.selected_webhook,
            View::Events => self.selected_event,
        }
    }

    pub fn set_selected(&mut self, idx: Option<usize>) {
        self.detail_scroll = 0;
        match self.view {
            View::Tasks => {
                self.selected_task = idx;
                self.task_list_state.select(idx);
            }
            View::Agents => self.selected_agent = idx,
            View::Knowledge => self.selected_knowledge = idx,
            View::Decisions => self.selected_decision = idx,
            View::Playbooks => self.selected_playbook = idx,
            View::Work => {
                self.selected_work = idx;
                self.work_list_state.select(idx);
            }
            View::Observations => self.selected_observation = idx,
            View::Team => self.selected_role = idx,
            View::Integrations => self.selected_integration = idx,
            View::Audit => self.selected_audit = idx,
            View::Logs => {}            // Logs use scroll, not selection
            View::ProjectSettings => {} // No list in settings view
            View::Verifications => self.selected_verification = idx,
            View::Git => self.selected_branch = idx,
            View::Search => self.selected_search_result = idx,
            View::Chat => {}
            View::Source => self.source_selected = idx,
            View::Dashboard | View::StepTemplates => {}
            View::Reports => self.selected_report = idx,
            View::Webhooks => self.selected_webhook = idx,
            View::Events => self.selected_event = idx,
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.set_selected(Some(next));
    }

    pub fn scroll_detail(&mut self, delta: i32) {
        let new = self.detail_scroll as i32 + delta;
        self.detail_scroll = new.max(0) as u16;
    }

    pub fn default_playbook_index(&self) -> usize {
        self.playbooks
            .iter()
            .position(|pb| pb.metadata.get("default").and_then(|v| v.as_bool()) == Some(true))
            .map(|i| i + 1) // 0 = None, 1+ = playbook index
            .unwrap_or(0)
    }

    pub fn status_summary(&self) -> String {
        let active = self
            .tasks
            .iter()
            .filter(|t| !matches!(t.state.as_str(), "backlog" | "ready" | "done" | "cancelled"))
            .count();
        let ready = self.tasks.iter().filter(|t| t.state == "ready").count();
        let _idle_agents = self.agents.iter().filter(|a| a.status == "idle").count();
        let project_name = self
            .current_project
            .and_then(|pid| self.projects.iter().find(|p| p.id == pid))
            .map(|p| p.name.as_str())
            .unwrap_or("–");
        format!(
            "Connected: {}  Project: {} [{}/{}]  Tasks: {} ({} active, {} ready)",
            if self.connected { "✓" } else { "✗" },
            project_name,
            self.projects
                .iter()
                .position(|p| Some(p.id) == self.current_project)
                .map(|i| i + 1)
                .unwrap_or(0),
            self.projects.len(),
            self.tasks.len(),
            active,
            ready,
        )
    }

    pub fn selected_task_id(&self) -> Option<Uuid> {
        self.selected_task
            .and_then(|i| self.tasks.get(i).map(|t| t.id))
    }

    /// Returns indices of tasks matching the current search_query (case-insensitive).
    /// Matches against title, kind, and state.
    pub fn filtered_task_indices(&self) -> Vec<usize> {
        if self.search_query.is_empty() {
            return (0..self.tasks.len()).collect();
        }
        let q = self.search_query.to_lowercase();
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.title.to_lowercase().contains(&q)
                    || t.state.to_lowercase().contains(&q)
                    || t.kind.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Scroll the log view.
    pub fn scroll_logs(&mut self, delta: i32) {
        let new = self.log_scroll as i32 + delta;
        self.log_scroll = new.max(0) as u16;
    }

    /// Returns verifications filtered by current kind and status filters.
    pub fn filtered_verifications(&self) -> Vec<&Verification> {
        self.verifications
            .iter()
            .filter(|v| {
                if let Some(ref k) = self.verification_kind_filter {
                    if v.kind != *k {
                        return false;
                    }
                }
                if let Some(ref s) = self.verification_status_filter {
                    if v.status != *s {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Returns events filtered by current kind and severity filters.
    pub fn filtered_events(&self) -> Vec<&ProjectEvent> {
        self.events
            .iter()
            .filter(|e| {
                if let Some(ref k) = self.event_kind_filter {
                    if e.kind != *k {
                        return false;
                    }
                }
                if let Some(ref s) = self.event_severity_filter {
                    if e.severity != *s {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Move task selection respecting the search filter.
    pub fn move_task_selection(&mut self, delta: i32) {
        let visible = self.filtered_task_indices();
        if visible.is_empty() {
            return;
        }
        // Find current position in the filtered list
        let current_pos = self
            .selected_task
            .and_then(|sel| visible.iter().position(|&i| i == sel))
            .unwrap_or(0);
        let next_pos = (current_pos as i32 + delta).clamp(0, visible.len() as i32 - 1) as usize;
        let new_idx = visible[next_pos];
        self.detail_scroll = 0;
        self.selected_task = Some(new_idx);
        self.task_list_state.select(Some(next_pos));
    }
}
