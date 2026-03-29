use std::collections::HashMap;

use crate::client::{
    Agent, AuditEntry, BranchInfo, ChangedFile, ChatMessage, Decision, GitTaskStatus, Integration,
    IntegrationAccess, LogEntry, MainPushStatus, Observation, Project, ProjectEvent,
    ProjectMetrics, Report, SearchResult, Task, TaskComment, TaskDependencies, TaskUpdate,
    TreeEntry, Webhook, WebhookDelivery, Work, WorkComment, WorkProgress,
};
use ratatui::widgets::ListState;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Agents,
    Decisions,
    Work,
    Observations,
    Integrations,
    Audit,
    Logs,
    ProjectSettings,
    Git,
    Search,
    Chat,
    Source,
    Dashboard,
    Reports,
    Events,
    Webhooks,
}

/// View ordering matches navigation.json — core → operations → reference → tools → system.
pub const ALL_VIEWS: &[View] = &[
    // core
    View::Work,
    View::Dashboard,
    View::Decisions,
    // operations
    View::Observations,
    View::Agents,
    // reference
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
];

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Agents => "Agents",
            View::Decisions => "Decisions",
            View::Work => "Work",
            View::Observations => "Observations",
            View::Integrations => "Integrations",
            View::Audit => "Audit",
            View::Logs => "Logs",
            View::ProjectSettings => "Project Settings",
            View::Git => "Git",
            View::Search => "Search",
            View::Chat => "Chat",
            View::Source => "Source",
            View::Dashboard => "Dashboard",
            View::Reports => "Reports",
            View::Events => "Events",
            View::Webhooks => "Webhooks",
        }
    }

    /// Keyboard shortcut — kept in sync with navigation.json.
    pub fn shortcut(self) -> &'static str {
        match self {
            View::Work => "1",
            View::Dashboard => "2",
            View::Decisions => "3",
            View::Observations => "4",
            View::Agents => "5",
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    None,
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

#[derive(Default)]
pub struct TaskForm {
    pub title: String,
    pub kind_index: usize,
    pub urgent: bool,
    pub spec: String,
    pub active_field: usize, // 0=title, 1=kind, 2=urgent, 3=spec
    pub cursor: usize,
    pub work_id: Option<Uuid>, // pre-linked work item (set when creating from Work view)
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
///         4=default_branch, 5=service_name, 6=claude_md
pub const SETTINGS_FIELD_COUNT: usize = 7;

pub struct ProjectSettingsForm {
    pub name: String,
    pub description: String,
    pub repo_url: String,
    pub repo_path: String,
    pub default_branch: String,
    pub service_name: String,
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
            6 => &self.claude_md,
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
            6 => Some(&mut self.claude_md),
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

#[derive(Default)]
pub struct WorkForm {
    pub title: String,
    pub description: String,
    pub success_criteria: String,
    pub status_index: usize,    // index into WORK_STATUSES
    pub work_type_index: usize, // index into WORK_TYPES
    pub auto_status: bool,
    pub active_field: usize, // 0=status, 1=title, 2=desc, 3=criteria, 4=type, 5=auto_status
    pub cursor: usize,
    pub editing_id: Option<Uuid>, // None for create, Some for edit
}

pub const CHAT_MODELS: &[&str] = &["sonnet", "opus", "haiku"];

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
    pub settings_form: Option<ProjectSettingsForm>,
    pub work_form: Option<WorkForm>,
    pub observation_form: Option<ObservationForm>,
    pub decision_form: Option<DecisionForm>,
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
    pub decisions: Vec<Decision>,
    pub work_items: Vec<Work>,
    pub done_work_items: Vec<Work>,
    pub done_work_page: usize,
    pub done_work_has_more: bool,
    pub work_section: usize, // 0 = active/paused, 1 = done/abandoned
    pub selected_done_work: Option<usize>,
    pub done_work_list_state: ListState,
    pub work_progress: Option<WorkProgress>,
    pub work_progress_map: HashMap<Uuid, WorkProgress>,
    pub work_children: Vec<Work>,
    pub observations: Vec<Observation>,
    pub integrations: Vec<Integration>,
    pub integration_access: Vec<IntegrationAccess>,
    pub audit_log: Vec<AuditEntry>,
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

    // Agent tasks (queue view)
    pub agent_tasks: Vec<Task>,

    // Work tasks
    pub work_tasks: Vec<Task>,
    pub work_task_selected: Option<usize>,
    pub work_task_list_state: ListState,
    pub work_task_updates: Vec<TaskUpdate>,
    pub work_task_comments: Vec<TaskComment>,
    pub work_task_detail_scroll: u16,
    /// Focus within Work view: 0=work list, 1=task list
    pub work_focus: usize,

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
    pub selected_decision: Option<usize>,
    pub selected_work: Option<usize>,
    pub work_list_state: ListState,
    pub selected_observation: Option<usize>,
    pub selected_integration: Option<usize>,
    pub selected_audit: Option<usize>,
    pub selected_event: Option<usize>,
    pub selected_webhook: Option<usize>,

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

    // Task hierarchy (used by work view)
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
    pub chat_model_index: usize, // index into CHAT_MODELS

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
            settings_form: None,
            work_form: None,
            observation_form: None,
            decision_form: None,
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
            decisions: vec![],
            work_items: vec![],
            done_work_items: vec![],
            done_work_page: 0,
            done_work_has_more: false,
            work_section: 0,
            selected_done_work: None,
            done_work_list_state: ListState::default(),
            work_progress: None,
            work_progress_map: HashMap::new(),
            work_children: vec![],
            observations: vec![],
            integrations: vec![],
            integration_access: vec![],
            audit_log: vec![],
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
            agent_tasks: vec![],
            work_tasks: vec![],
            work_task_selected: None,
            work_task_list_state: ListState::default(),
            work_task_updates: vec![],
            work_task_comments: vec![],
            work_task_detail_scroll: 0,
            work_focus: 0,
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
            selected_decision: None,
            selected_work: None,
            work_list_state: ListState::default(),
            selected_observation: None,
            selected_integration: None,
            selected_audit: None,
            selected_event: None,
            selected_webhook: None,
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
            chat_model_index: 0, // default to "sonnet"
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
            View::Agents => self.agents.len(),
            View::Decisions => self.decisions.len(),
            View::Work => {
                if self.work_section == 0 {
                    self.work_items.len()
                } else {
                    self.done_work_items.len()
                }
            }
            View::Observations => self.observations.len(),
            View::Integrations => self.integrations.len(),
            View::Audit => self.audit_log.len(),
            View::Logs => self.log_entries.len(),
            View::ProjectSettings => 0, // No list in settings view
            View::Git => self.branches.len(),
            View::Search => self.search_results.len(),
            View::Chat => self.chat_messages.len(),
            View::Source => self.source_entries.len(),
            View::Dashboard => 0,
            View::Reports => self.reports.len(),
            View::Webhooks => self.webhooks.len(),
            View::Events => self.filtered_events().len(),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        match self.view {
            View::Agents => self.selected_agent,
            View::Decisions => self.selected_decision,
            View::Work => {
                if self.work_section == 0 {
                    self.selected_work
                } else {
                    self.selected_done_work
                }
            }
            View::Observations => self.selected_observation,
            View::Integrations => self.selected_integration,
            View::Audit => self.selected_audit,
            View::Logs => None, // Logs use scroll, not selection
            View::ProjectSettings => None,
            View::Git => self.selected_branch,
            View::Search => self.selected_search_result,
            View::Chat => None,
            View::Source => self.source_selected,
            View::Dashboard => None,
            View::Reports => self.selected_report,
            View::Webhooks => self.selected_webhook,
            View::Events => self.selected_event,
        }
    }

    pub fn set_selected(&mut self, idx: Option<usize>) {
        self.detail_scroll = 0;
        match self.view {
            View::Agents => self.selected_agent = idx,
            View::Decisions => self.selected_decision = idx,
            View::Work => {
                if self.work_section == 0 {
                    self.selected_work = idx;
                    self.work_list_state.select(idx);
                } else {
                    self.selected_done_work = idx;
                    self.done_work_list_state.select(idx);
                }
            }
            View::Observations => self.selected_observation = idx,
            View::Integrations => self.selected_integration = idx,
            View::Audit => self.selected_audit = idx,
            View::Logs => {}            // Logs use scroll, not selection
            View::ProjectSettings => {} // No list in settings view
            View::Git => self.selected_branch = idx,
            View::Search => self.selected_search_result = idx,
            View::Chat => {}
            View::Source => self.source_selected = idx,
            View::Dashboard => {}
            View::Reports => self.selected_report = idx,
            View::Webhooks => self.selected_webhook = idx,
            View::Events => self.selected_event = idx,
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.view == View::Work {
            self.move_work_selection(delta);
            return;
        }
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.set_selected(Some(next));
    }

    fn move_work_selection(&mut self, delta: i32) {
        self.detail_scroll = 0;
        if self.work_section == 0 {
            let len = self.work_items.len();
            if len == 0 && !self.done_work_items.is_empty() && delta > 0 {
                // Jump to done section
                self.work_section = 1;
                self.selected_done_work = Some(0);
                self.done_work_list_state.select(Some(0));
                return;
            }
            if len == 0 {
                return;
            }
            let current = self.selected_work.unwrap_or(0) as i32;
            let next = current + delta;
            if next >= len as i32 && !self.done_work_items.is_empty() {
                // Move to done section
                self.work_section = 1;
                self.selected_done_work = Some(0);
                self.done_work_list_state.select(Some(0));
            } else {
                let clamped = next.clamp(0, len as i32 - 1) as usize;
                self.selected_work = Some(clamped);
                self.work_list_state.select(Some(clamped));
            }
        } else {
            let len = self.done_work_items.len();
            if len == 0 && !self.work_items.is_empty() && delta < 0 {
                // Jump to active section
                self.work_section = 0;
                let last = self.work_items.len().saturating_sub(1);
                self.selected_work = Some(last);
                self.work_list_state.select(Some(last));
                return;
            }
            if len == 0 {
                return;
            }
            let current = self.selected_done_work.unwrap_or(0) as i32;
            let next = current + delta;
            if next < 0 && !self.work_items.is_empty() {
                // Move to active section
                self.work_section = 0;
                let last = self.work_items.len().saturating_sub(1);
                self.selected_work = Some(last);
                self.work_list_state.select(Some(last));
            } else {
                let clamped = next.clamp(0, len as i32 - 1) as usize;
                self.selected_done_work = Some(clamped);
                self.done_work_list_state.select(Some(clamped));
            }
        }
    }

    /// Move the task selection within the work task list.
    pub fn move_work_task_selection(&mut self, delta: i32) {
        let len = self.work_tasks.len();
        if len == 0 {
            return;
        }
        let current = self.work_task_selected.unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.work_task_selected = Some(next);
        self.work_task_list_state.select(Some(next));
        self.work_task_detail_scroll = 0;
    }

    /// Returns the currently selected work item from either section.
    pub fn selected_work_item(&self) -> Option<&Work> {
        if self.work_section == 0 {
            self.selected_work.and_then(|i| self.work_items.get(i))
        } else {
            self.selected_done_work
                .and_then(|i| self.done_work_items.get(i))
        }
    }

    pub fn scroll_detail(&mut self, delta: i32) {
        let new = self.detail_scroll as i32 + delta;
        self.detail_scroll = new.max(0) as u16;
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
}
