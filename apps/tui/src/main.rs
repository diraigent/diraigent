mod app;
mod client;
mod theme;
mod views;
mod widgets;

use std::io;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::Widget;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tokio::sync::{mpsc, watch};

use app::{
    App, Modal, VerificationForm, View, ALL_VIEWS, EVENT_KINDS, EVENT_SEVERITIES, GOAL_STATUSES,
    GOAL_TYPES, INTEGRATION_AUTH_TYPES, INTEGRATION_KINDS, KNOWLEDGE_CATEGORIES, LOG_DIRECTIONS,
    LOG_LIMITS, OBSERVATION_KINDS, OBSERVATION_SEVERITIES, TASK_KINDS, TIME_RANGES,
    VERIFICATION_KINDS, VERIFICATION_STATUSES,
};
use client::ApiClient;

#[derive(Debug)]
enum ApiMsg {
    Connected(bool),
    Projects(Vec<client::Project>),
    Tasks(Vec<client::Task>),
    TaskUpdates(Vec<client::TaskUpdate>),
    TaskComments(Vec<client::TaskComment>),
    TaskDependencies(client::TaskDependencies),
    Agents(Vec<client::Agent>),
    Knowledge(Vec<client::KnowledgeEntry>),
    Decisions(Vec<client::Decision>),
    Playbooks(Vec<client::Playbook>),
    Goals(Vec<client::Goal>),
    GoalProgress(client::GoalProgress),
    GoalStats(client::GoalStats),
    GoalChildren(Vec<client::Goal>),
    Observations(Vec<client::Observation>),
    Roles(Vec<client::Role>),
    Members(Vec<client::Member>),
    Integrations(Vec<client::Integration>),
    IntegrationAccessList(Vec<client::IntegrationAccess>),
    Audit(Vec<client::AuditEntry>),
    Logs(client::LogsResponse),
    LogLabels(Vec<String>),
    ClaudeMd(String),
    ProjectUpdated(client::Project),
    GitTaskStatus(client::GitTaskStatus),
    ChangedFiles(Vec<client::ChangedFile>),
    Verifications(Vec<client::Verification>),
    Events(Vec<client::Event>),
    Reports(Vec<client::Report>),
    Webhooks(Vec<client::Webhook>),
    WebhookDeliveries(Vec<client::WebhookDelivery>),
    WebhookTestResult(String),
    GoalTasksList(Vec<client::Task>),
    GoalUnlinkedTasks(Vec<client::Task>),
    GoalBulkLinked,
    Branches(client::BranchListResponse),
    MainStatus(client::MainPushStatus),
    GitActionResult(String),
    SearchResults(client::SearchResponse),
    ChatResponse(String),
    ChatError(String),
    SourceTree(Vec<client::TreeEntry>),
    SourceBlob { path: String, content: String },
    GoalComments(Vec<client::GoalComment>),
    StepTemplates(Vec<client::StepTemplate>),
    AgentTasks(Vec<client::Task>),
    ObservationsCleanup(client::CleanupObservationsResult),
    Error(String),
}

fn valid_transitions(state: &str) -> Vec<String> {
    match state {
        "backlog" => vec!["ready".into(), "cancelled".into()],
        "ready" => vec!["cancelled".into()],
        "done" => vec!["ready".into()],
        "cancelled" => vec!["backlog".into()],
        // Any active step (implement, review, merge, working, etc.)
        _ => vec!["done".into(), "ready".into(), "cancelled".into()],
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let api = ApiClient::new();
    let (tx, mut rx) = mpsc::channel::<ApiMsg>(64);
    let (project_tx, project_rx) = watch::channel::<Option<uuid::Uuid>>(None);

    // Initial fetch
    {
        let api = api.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let connected = api.health().await.unwrap_or(false);
            let _ = tx.send(ApiMsg::Connected(connected)).await;
            if let Ok(projects) = api.list_projects().await {
                let _ = tx.send(ApiMsg::Projects(projects)).await;
            }
            if let Ok(agents) = api.list_agents().await {
                let _ = tx.send(ApiMsg::Agents(agents)).await;
            }
        });
    }

    // Shared helper: fetch all project data in parallel
    async fn fetch_project_data(api: &ApiClient, tx: &mpsc::Sender<ApiMsg>, pid: uuid::Uuid) {
        let (
            tasks,
            playbooks,
            knowledge,
            decisions,
            goals,
            observations,
            roles,
            members,
            integrations,
            audit,
            verifications,
            events,
            reports,
            webhooks,
        ) = tokio::join!(
            api.list_tasks(pid),
            api.list_playbooks(pid),
            api.list_knowledge(pid),
            api.list_decisions(pid),
            api.list_goals(pid),
            api.list_observations(pid),
            api.list_roles(),
            api.list_members(),
            api.list_integrations(pid),
            api.list_audit(pid),
            api.list_verifications(pid, None, None, None, 100, 0),
            api.list_events(pid, None, None),
            api.list_reports(pid),
            api.list_webhooks(pid),
        );
        if let Ok(resp) = tasks {
            let _ = tx.send(ApiMsg::Tasks(resp.data)).await;
        }
        if let Ok(resp) = playbooks {
            let _ = tx.send(ApiMsg::Playbooks(resp)).await;
        }
        if let Ok(resp) = knowledge {
            let _ = tx.send(ApiMsg::Knowledge(resp)).await;
        }
        if let Ok(resp) = decisions {
            let _ = tx.send(ApiMsg::Decisions(resp)).await;
        }
        if let Ok(resp) = goals {
            let _ = tx.send(ApiMsg::Goals(resp)).await;
        }
        if let Ok(resp) = observations {
            let _ = tx.send(ApiMsg::Observations(resp)).await;
        }
        if let Ok(resp) = roles {
            let _ = tx.send(ApiMsg::Roles(resp)).await;
        }
        if let Ok(resp) = members {
            let _ = tx.send(ApiMsg::Members(resp)).await;
        }
        if let Ok(resp) = integrations {
            let _ = tx.send(ApiMsg::Integrations(resp)).await;
        }
        if let Ok(resp) = audit {
            let _ = tx.send(ApiMsg::Audit(resp)).await;
        }
        if let Ok(resp) = verifications {
            let _ = tx.send(ApiMsg::Verifications(resp)).await;
        }
        if let Ok(resp) = events {
            let _ = tx.send(ApiMsg::Events(resp)).await;
        }
        if let Ok(resp) = reports {
            let _ = tx.send(ApiMsg::Reports(resp)).await;
        }
        if let Ok(resp) = webhooks {
            let _ = tx.send(ApiMsg::Webhooks(resp)).await;
        }
    }

    // Immediate fetch when project changes
    {
        let api = api.clone();
        let tx = tx.clone();
        let mut project_rx = project_rx.clone();
        tokio::spawn(async move {
            // Skip the initial value
            {
                let _ = project_rx.borrow_and_update();
            }
            while project_rx.changed().await.is_ok() {
                let pid = { *project_rx.borrow_and_update() };
                if let Some(pid) = pid {
                    fetch_project_data(&api, &tx, pid).await;
                }
            }
        });
    }

    // Polling loop (background refresh)
    {
        let api = api.clone();
        let tx = tx.clone();
        let project_rx = project_rx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                let (connected, agents) = tokio::join!(api.health(), api.list_agents());
                let _ = tx.send(ApiMsg::Connected(connected.unwrap_or(false))).await;
                if let Ok(agents) = agents {
                    let _ = tx.send(ApiMsg::Agents(agents)).await;
                }
                let pid = { *project_rx.borrow() };
                if let Some(pid) = pid {
                    fetch_project_data(&api, &tx, pid).await;
                }
            }
        });
    }

    let mut last_nav_time: Option<Instant> = None;
    let mut last_fetched_task: Option<uuid::Uuid> = None;

    loop {
        // Debounced detail fetch: if selection changed and 200ms passed
        if let (Some(nav_time), Some(tid)) = (last_nav_time, app.selected_task_id()) {
            if nav_time.elapsed() >= Duration::from_millis(200) && Some(tid) != last_fetched_task {
                last_fetched_task = Some(tid);
                last_nav_time = None;
                let api = api.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let (updates, comments, deps, git_status) = tokio::join!(
                        api.get_task_updates(tid),
                        api.get_task_comments(tid),
                        api.list_task_dependencies(tid),
                        api.get_git_task_status(tid),
                    );
                    if let Ok(updates) = updates {
                        let _ = tx.send(ApiMsg::TaskUpdates(updates)).await;
                    }
                    if let Ok(comments) = comments {
                        let _ = tx.send(ApiMsg::TaskComments(comments)).await;
                    }
                    if let Ok(deps) = deps {
                        let _ = tx.send(ApiMsg::TaskDependencies(deps)).await;
                    }
                    if let Ok(status) = git_status {
                        let _ = tx.send(ApiMsg::GitTaskStatus(status)).await;
                    }
                });
            }
        }

        // Process API messages
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ApiMsg::Connected(c) => app.connected = c,
                ApiMsg::Projects(p) => {
                    app.projects = p;
                    if app.current_project.is_none() {
                        if let Some(first) = app.projects.first() {
                            app.current_project = Some(first.id);
                            project_tx.send(Some(first.id)).ok();
                        }
                    }
                }
                ApiMsg::Tasks(mut t) => {
                    // Hide done/cancelled tasks older than N days (from project metadata, default 1)
                    let retention_days: i64 = app
                        .current_project
                        .and_then(|pid| app.projects.iter().find(|p| p.id == pid))
                        .and_then(|p| p.metadata.get("done_retention_days"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1);
                    let cutoff = Utc::now() - chrono::Duration::days(retention_days);
                    t.retain(|task| {
                        if task.state == "done" || task.state == "cancelled" {
                            task.updated_at
                                .as_deref()
                                .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                                .map(|dt| dt > cutoff)
                                .unwrap_or(true) // keep if we can't parse the date
                        } else {
                            true
                        }
                    });

                    // Sort: done/cancelled sink to bottom, ordered by completion time (most recent first)
                    t.sort_by(|a, b| {
                        let group = |t: &client::Task| -> u8 {
                            match t.state.as_str() {
                                "done" | "cancelled" => 1,
                                _ => 0,
                            }
                        };
                        let ga = group(a);
                        let gb = group(b);
                        ga.cmp(&gb).then_with(|| {
                            if ga == 1 {
                                // Both done/cancelled: sort by completed_at descending (most recent first)
                                let da = a
                                    .completed_at
                                    .as_deref()
                                    .or(a.updated_at.as_deref())
                                    .and_then(|s| s.parse::<DateTime<Utc>>().ok());
                                let db = b
                                    .completed_at
                                    .as_deref()
                                    .or(b.updated_at.as_deref())
                                    .and_then(|s| s.parse::<DateTime<Utc>>().ok());
                                db.cmp(&da) // descending
                            } else {
                                std::cmp::Ordering::Equal // preserve original order for active tasks
                            }
                        })
                    });

                    // Preserve selection by task ID across refreshes
                    let prev_id = app.selected_task_id();
                    app.tasks = t;

                    if let Some(pid) = prev_id {
                        app.selected_task = app.tasks.iter().position(|t| t.id == pid);
                    }
                    if app.selected_task.is_none() && !app.tasks.is_empty() {
                        app.selected_task = Some(0);
                    }
                    app.task_list_state.select(app.selected_task);

                    // Fetch updates + comments for selected task if none loaded yet
                    if app.task_updates.is_empty() {
                        if let Some(tid) = app.selected_task_id() {
                            let api = api.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let (updates, comments) = tokio::join!(
                                    api.get_task_updates(tid),
                                    api.get_task_comments(tid),
                                );
                                if let Ok(updates) = updates {
                                    let _ = tx.send(ApiMsg::TaskUpdates(updates)).await;
                                }
                                if let Ok(comments) = comments {
                                    let _ = tx.send(ApiMsg::TaskComments(comments)).await;
                                }
                            });
                        }
                    }
                }
                ApiMsg::TaskUpdates(u) => app.task_updates = u,
                ApiMsg::TaskComments(c) => app.task_comments = c,
                ApiMsg::TaskDependencies(d) => app.task_dependencies = d,
                ApiMsg::Agents(a) => app.agents = a,
                ApiMsg::Knowledge(k) => app.knowledge = k,
                ApiMsg::Decisions(d) => app.decisions = d,
                ApiMsg::Playbooks(p) => app.playbooks = p,
                ApiMsg::Goals(g) => app.goals = g,
                ApiMsg::GoalProgress(p) => app.goal_progress = Some(p),
                ApiMsg::GoalStats(s) => app.goal_stats = Some(s),
                ApiMsg::GoalChildren(c) => app.goal_children = c,
                ApiMsg::Observations(o) => app.observations = o,
                ApiMsg::Roles(r) => app.roles = r,
                ApiMsg::Members(m) => app.members = m,
                ApiMsg::Integrations(intg) => app.integrations = intg,
                ApiMsg::IntegrationAccessList(access) => app.integration_access = access,
                ApiMsg::Audit(a) => app.audit_log = a,
                ApiMsg::Verifications(v) => app.verifications = v,
                ApiMsg::Events(e) => app.events = e,
                ApiMsg::Reports(r) => app.reports = r,
                ApiMsg::Webhooks(w) => app.webhooks = w,
                ApiMsg::WebhookDeliveries(d) => app.webhook_deliveries = d,
                ApiMsg::WebhookTestResult(r) => app.webhook_test_result = Some(r),
                ApiMsg::GoalTasksList(tasks) => app.goal_tasks = tasks,
                ApiMsg::GoalUnlinkedTasks(tasks) => {
                    app.goal_unlinked_tasks = tasks;
                    app.goal_picker_loading = false;
                }
                ApiMsg::GoalBulkLinked => {
                    // Refresh goal tasks + progress after linking
                    if let Some(goal) = app.selected_goal.and_then(|i| app.goals.get(i)) {
                        let gid = goal.id;
                        let api = api.clone();
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Ok(tasks) = api.list_goal_tasks(gid, 50, 0).await {
                                let _ = tx.send(ApiMsg::GoalTasksList(tasks)).await;
                            }
                            if let Ok(progress) = api.get_goal_progress(gid).await {
                                let _ = tx.send(ApiMsg::GoalProgress(progress)).await;
                            }
                            if let Ok(stats) = api.get_goal_stats(gid).await {
                                let _ = tx.send(ApiMsg::GoalStats(stats)).await;
                            }
                        });
                    }
                }
                ApiMsg::Logs(resp) => {
                    app.log_entries = resp.entries;
                    app.log_loading = false;
                    app.log_scroll = 0;
                }
                ApiMsg::LogLabels(labels) => app.log_labels = labels,
                ApiMsg::ClaudeMd(content) => {
                    if let Some(ref mut form) = app.settings_form {
                        form.claude_md = content;
                    }
                }
                ApiMsg::ProjectUpdated(proj) => {
                    // Update project in the list
                    if let Some(existing) = app.projects.iter_mut().find(|p| p.id == proj.id) {
                        *existing = proj;
                    }
                }
                ApiMsg::GitTaskStatus(status) => app.git_task_status = Some(status),
                ApiMsg::ChangedFiles(files) => app.changed_files = files,
                ApiMsg::Branches(resp) => {
                    app.current_branch = resp.current_branch;
                    app.branches = resp.branches;
                    if app.selected_branch.is_none() && !app.branches.is_empty() {
                        app.selected_branch = Some(0);
                    }
                }
                ApiMsg::MainStatus(status) => app.main_push_status = Some(status),
                ApiMsg::GitActionResult(msg) => app.git_action_result = Some(msg),
                ApiMsg::SearchResults(resp) => {
                    app.search_results = resp.results;
                    app.search_total = resp.total;
                    app.search_executed_query = resp.query;
                    if !app.search_results.is_empty() {
                        app.selected_search_result = Some(0);
                    }
                }
                ApiMsg::ChatResponse(content) => {
                    app.chat_streaming = false;
                    if !content.is_empty() {
                        app.chat_messages.push(client::ChatMessage {
                            role: "assistant".into(),
                            content,
                        });
                    }
                }
                ApiMsg::ChatError(e) => {
                    app.chat_streaming = false;
                    app.chat_messages.push(client::ChatMessage {
                        role: "assistant".into(),
                        content: format!("Error: {}", e),
                    });
                }
                ApiMsg::SourceTree(entries) => {
                    app.source_entries = entries;
                    if !app.source_entries.is_empty() {
                        let offset = if app.source_current_path.is_empty() {
                            0
                        } else {
                            1
                        };
                        app.source_selected = Some(offset);
                    } else {
                        app.source_selected = if app.source_current_path.is_empty() {
                            None
                        } else {
                            Some(0)
                        };
                    }
                    app.source_blob_content = None;
                    app.source_blob_path = None;
                }
                ApiMsg::SourceBlob { path, content } => {
                    app.source_blob_content = Some(content);
                    app.source_blob_path = Some(path);
                    app.source_blob_scroll = 0;
                }
                ApiMsg::GoalComments(comments) => {
                    app.goal_comments = comments;
                }
                ApiMsg::StepTemplates(templates) => {
                    app.step_templates = templates;
                }
                ApiMsg::AgentTasks(tasks) => {
                    app.agent_tasks = tasks;
                }
                ApiMsg::ObservationsCleanup(result) => {
                    app.last_error = Some(format!(
                        "Cleanup: {} deleted (dismissed={}, ack={}, acted={}, resolved={}, dup={})",
                        result.total_deleted,
                        result.deleted_dismissed,
                        result.deleted_acknowledged,
                        result.deleted_acted_on,
                        result.deleted_resolved,
                        result.deleted_duplicates,
                    ));
                    // Refresh observations list
                    if let Some(pid) = app.current_project {
                        let api = api.clone();
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Ok(obs) = api.list_observations(pid).await {
                                let _ = tx.send(ApiMsg::Observations(obs)).await;
                            }
                        });
                    }
                }
                ApiMsg::Error(e) => app.last_error = Some(e),
            }
        }

        // Render
        terminal.draw(|f| {
            let size = f.area();
            let main_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Status bar
                    Constraint::Min(5),    // Main content
                    Constraint::Length(1), // Footer
                ])
                .split(size);

            // Status bar
            let status = app.status_summary();
            let conn_color = if app.connected {
                theme::green()
            } else {
                theme::red()
            };
            let status_block = Block::default()
                .title(" Stareto ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::mauve()));
            let status_spans = if let Some(ref err) = app.last_error {
                vec![
                    Span::styled(" ERROR: ", Style::default().fg(theme::red())),
                    Span::styled(err.clone(), Style::default().fg(theme::red())),
                ]
            } else {
                vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(status, Style::default().fg(conn_color)),
                ]
            };
            let status_p = Paragraph::new(Line::from(status_spans))
                .block(status_block)
                .style(Style::default().bg(theme::mantle()));
            f.render_widget(status_p, main_layout[0]);

            // Main content
            match app.view {
                View::Tasks => views::tasks::render(f, main_layout[1], &mut app),
                View::Agents => views::agents::render(f, main_layout[1], &mut app),
                View::Knowledge => views::knowledge::render(f, main_layout[1], &mut app),
                View::Decisions => views::decisions::render(f, main_layout[1], &mut app),
                View::Playbooks => views::playbooks::render(f, main_layout[1], &mut app),
                View::Goals => views::goals::render(f, main_layout[1], &mut app),
                View::Observations => views::observations::render(f, main_layout[1], &mut app),
                View::Team => views::team::render(f, main_layout[1], &mut app),
                View::Integrations => views::integrations::render(f, main_layout[1], &mut app),
                View::Audit => views::audit::render(f, main_layout[1], &mut app),
                View::Logs => views::logs::render(f, main_layout[1], &mut app),
                View::ProjectSettings => {
                    views::project_settings::render(f, main_layout[1], &mut app)
                }
                View::Verifications => views::verifications::render(f, main_layout[1], &mut app),
                View::Git => views::git::render(f, main_layout[1], &mut app),
                View::Search => views::search::render(f, main_layout[1], &mut app),
                View::Chat => views::chat::render(f, main_layout[1], &mut app),
                View::Source => views::source::render(f, main_layout[1], &mut app),
                View::Events => views::events::render(f, main_layout[1], &mut app),
                View::Webhooks => views::webhooks::render(f, main_layout[1], &mut app),
                View::Reports => views::reports::render(f, main_layout[1], &mut app),
                View::Dashboard | View::StepTemplates => {
                    let label = app.view.label();
                    let block = ratatui::widgets::Block::default()
                        .title(format!(" {} ", label))
                        .borders(ratatui::widgets::Borders::ALL)
                        .border_style(Style::default().fg(theme::overlay0()));
                    let text = Paragraph::new(format!("{} view — coming soon", label))
                        .block(block)
                        .style(Style::default().fg(theme::subtext0()));
                    f.render_widget(text, main_layout[1]);
                }
            }

            // Footer
            let footer = Paragraph::new(Line::from(vec![
                Span::styled(
                    " [1]",
                    Style::default().fg(if app.view == View::Tasks {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Tasks ", Style::default().fg(theme::text())),
                Span::styled(
                    "[2]",
                    Style::default().fg(if app.view == View::Agents {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Agents ", Style::default().fg(theme::text())),
                Span::styled(
                    "[3]",
                    Style::default().fg(if app.view == View::Knowledge {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Knowledge ", Style::default().fg(theme::text())),
                Span::styled(
                    "[4]",
                    Style::default().fg(if app.view == View::Decisions {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Decisions ", Style::default().fg(theme::text())),
                Span::styled(
                    "[5]",
                    Style::default().fg(if app.view == View::Playbooks {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Playbooks ", Style::default().fg(theme::text())),
                Span::styled(
                    "[6]",
                    Style::default().fg(if app.view == View::Goals {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Goals ", Style::default().fg(theme::text())),
                Span::styled(
                    "[7]",
                    Style::default().fg(if app.view == View::Observations {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Obs ", Style::default().fg(theme::text())),
                Span::styled(
                    "[8]",
                    Style::default().fg(if app.view == View::Team {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Team ", Style::default().fg(theme::text())),
                Span::styled(
                    "[9]",
                    Style::default().fg(if app.view == View::Integrations {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Int ", Style::default().fg(theme::text())),
                Span::styled(
                    "[0]",
                    Style::default().fg(if app.view == View::Audit {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Audit ", Style::default().fg(theme::text())),
                Span::styled(
                    "[l]",
                    Style::default().fg(if app.view == View::Logs {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Logs ", Style::default().fg(theme::text())),
                Span::styled(
                    "[S]",
                    Style::default().fg(if app.view == View::ProjectSettings {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Settings ", Style::default().fg(theme::text())),
                Span::styled(
                    "[V]",
                    Style::default().fg(if app.view == View::Verifications {
                        theme::blue()
                    } else {
                        theme::overlay0()
                    }),
                ),
                Span::styled("Verify ", Style::default().fg(theme::text())),
                Span::styled("[`]", Style::default().fg(theme::overlay0())),
                Span::styled("Views ", Style::default().fg(theme::text())),
                Span::styled("[[]", Style::default().fg(theme::overlay0())),
                Span::styled("[]]", Style::default().fg(theme::overlay0())),
                Span::styled("Project ", Style::default().fg(theme::text())),
                // View-specific shortcuts
                if app.view == View::Tasks {
                    Span::styled("[f]Flag [F]Files [c]Comment ", Style::default().fg(theme::overlay0()))
                } else if app.view == View::Goals {
                    Span::styled("[c]Comment [l]Link ", Style::default().fg(theme::overlay0()))
                } else if app.view == View::Observations {
                    Span::styled("[C]Cleanup [p]Promote ", Style::default().fg(theme::overlay0()))
                } else if app.view == View::Playbooks {
                    Span::styled("[T]Templates ", Style::default().fg(theme::overlay0()))
                } else if app.view == View::Agents {
                    Span::styled("[a]Tasks ", Style::default().fg(theme::overlay0()))
                } else {
                    Span::styled("", Style::default())
                },
                Span::styled("[/]", Style::default().fg(theme::overlay0())),
                Span::styled("Search ", Style::default().fg(theme::text())),
                Span::styled("[q]", Style::default().fg(theme::overlay0())),
                Span::styled("Quit ", Style::default().fg(theme::text())),
                Span::styled("[L]", Style::default().fg(theme::overlay0())),
                Span::styled(
                    if theme::is_light() { "Light" } else { "Dark" },
                    Style::default().fg(theme::text()),
                ),
            ]))
            .style(Style::default().bg(theme::mantle()));
            f.render_widget(footer, main_layout[2]);

            // Modals
            match app.modal {
                Modal::Transition => {
                    let states: Vec<&str> =
                        app.transition_options.iter().map(|s| s.as_str()).collect();
                    f.render_widget(
                        widgets::popup::TransitionPopup {
                            states: &states,
                            selected: app.transition_selected,
                        },
                        size,
                    );
                }
                Modal::Reply
                | Modal::Comment
                | Modal::GoalComment
                | Modal::Search
                | Modal::GlobalSearch
                | Modal::LogQuery
                | Modal::LogFilter => {
                    let title = if app.modal == Modal::LogQuery {
                        "LogQL Query"
                    } else if app.modal == Modal::LogFilter {
                        "Filter Log Output"
                    } else if app.modal == Modal::Search {
                        "Search"
                    } else if app.modal == Modal::GlobalSearch {
                        "Global Search"
                    } else if app.modal == Modal::Comment {
                        "Comment"
                    } else if app.modal == Modal::GoalComment {
                        "Goal Comment"
                    } else if app
                        .selected_task
                        .and_then(|i| app.tasks.get(i))
                        .is_some_and(|t| t.state == "backlog")
                    {
                        "Refine spec"
                    } else {
                        "Reply"
                    };
                    f.render_widget(
                        widgets::popup::InputPopup {
                            title,
                            text: &app.input_text,
                            cursor: app.input_cursor,
                        },
                        size,
                    );
                }
                Modal::ChatInput => {
                    f.render_widget(
                        widgets::popup::InputPopup {
                            title: "Chat Message",
                            text: &app.chat_input,
                            cursor: app.chat_input.len(),
                        },
                        size,
                    );
                }
                Modal::ClaimTask | Modal::DelegateTask | Modal::BulkTransition => {
                    let states: Vec<&str> =
                        app.transition_options.iter().map(|s| s.as_str()).collect();
                    let title = match app.modal {
                        Modal::ClaimTask => " Claim Task — Select Agent ",
                        Modal::DelegateTask => " Delegate Task — Select Agent ",
                        Modal::BulkTransition => " Bulk Transition ",
                        _ => "",
                    };
                    let popup_area = widgets::popup::centered_rect(40, 50, size);
                    Clear.render(popup_area, f.buffer_mut());
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::peach()))
                        .style(Style::default().bg(theme::mantle()));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);
                    let items: Vec<ratatui::widgets::ListItem> = states
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let style = if i == app.transition_selected {
                                Style::default().fg(theme::base()).bg(theme::peach())
                            } else {
                                Style::default().fg(theme::text())
                            };
                            ratatui::widgets::ListItem::new(Line::styled(
                                format!("  {}  ", s),
                                style,
                            ))
                        })
                        .collect();
                    f.render_widget(ratatui::widgets::List::new(items), inner);
                }
                Modal::BulkDelete | Modal::ReportDelete => {
                    let count = app.bulk_selected.len();
                    let popup_area = widgets::popup::centered_rect(50, 20, size);
                    Clear.render(popup_area, f.buffer_mut());
                    let block = Block::default()
                        .title(" Confirm Bulk Delete ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::red()))
                        .style(Style::default().bg(theme::mantle()));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);
                    let msg = format!(
                        "  Delete {} task{}? [y/Enter=yes  n/Esc=no]",
                        count,
                        if count == 1 { "" } else { "s" }
                    );
                    let p = Paragraph::new(Line::styled(
                        msg,
                        Style::default().fg(theme::red()),
                    ));
                    f.render_widget(p, inner);
                }
                Modal::ObservationStatus
                | Modal::DecisionSupersede
                | Modal::IntegrationAccess
                | Modal::ViewPicker
                | Modal::VerificationStatus
                | Modal::VerificationKindFilter
                | Modal::VerificationStatusFilter
                | Modal::EventKindFilter
                | Modal::EventSeverityFilter => {
                    let states: Vec<&str> =
                        app.transition_options.iter().map(|s| s.as_str()).collect();
                    let title = match app.modal {
                        Modal::ObservationStatus => " Observation Status ",
                        Modal::DecisionSupersede => " Supersede with Decision ",
                        Modal::IntegrationAccess => " Agent Access ",
                        Modal::ViewPicker => " Switch View ",
                        Modal::VerificationStatus => " Verification Status ",
                        Modal::VerificationKindFilter => " Filter by Kind ",
                        Modal::VerificationStatusFilter => " Filter by Status ",
                        Modal::EventKindFilter => " Filter Events by Kind ",
                        Modal::EventSeverityFilter => " Filter Events by Severity ",
                        _ => "",
                    };
                    let popup_area = widgets::popup::centered_rect(40, 50, size);
                    Clear.render(popup_area, f.buffer_mut());
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::teal()))
                        .style(Style::default().bg(theme::mantle()));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);
                    let items: Vec<ratatui::widgets::ListItem> = states
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let style = if i == app.transition_selected {
                                Style::default().fg(theme::base()).bg(theme::teal())
                            } else {
                                Style::default().fg(theme::text())
                            };
                            ratatui::widgets::ListItem::new(Line::styled(
                                format!("  {}  ", s),
                                style,
                            ))
                        })
                        .collect();
                    f.render_widget(ratatui::widgets::List::new(items), inner);
                }
                Modal::GoalLink | Modal::GoalStatus | Modal::DependencyAdd => {
                    // Reuse transition popup pattern: show goals/statuses/tasks as list
                    let states: Vec<&str> =
                        app.transition_options.iter().map(|s| s.as_str()).collect();
                    let title = if app.modal == Modal::GoalLink {
                        " Link to Goal "
                    } else if app.modal == Modal::DependencyAdd {
                        " Add Dependency "
                    } else {
                        " Goal Status "
                    };
                    let popup_area = widgets::popup::centered_rect(40, 50, size);
                    Clear.render(popup_area, f.buffer_mut());
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::green()))
                        .style(Style::default().bg(theme::mantle()));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);
                    let items: Vec<ratatui::widgets::ListItem> = states
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let style = if i == app.transition_selected {
                                Style::default().fg(theme::base()).bg(theme::green())
                            } else {
                                Style::default().fg(theme::text())
                            };
                            ratatui::widgets::ListItem::new(Line::styled(
                                format!("  {}  ", s),
                                style,
                            ))
                        })
                        .collect();
                    f.render_widget(ratatui::widgets::List::new(items), inner);
                }
                Modal::GoalTaskPicker => {
                    let popup_area = widgets::popup::centered_rect(60, 70, size);
                    Clear.render(popup_area, f.buffer_mut());
                    let title = if app.goal_picker_loading {
                        " Link Tasks to Goal (loading...) "
                    } else {
                        " Link Tasks to Goal [Space=toggle  Enter=confirm  Esc=cancel] "
                    };
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::green()))
                        .style(Style::default().bg(theme::mantle()));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);
                    if app.goal_unlinked_tasks.is_empty() && !app.goal_picker_loading {
                        let p = Paragraph::new(Line::styled(
                            "  No unlinked tasks available.",
                            Style::default().fg(theme::overlay0()),
                        ));
                        f.render_widget(p, inner);
                    } else {
                        let items: Vec<ratatui::widgets::ListItem> = app
                            .goal_unlinked_tasks
                            .iter()
                            .enumerate()
                            .map(|(i, t)| {
                                let checked = if app.goal_picker_checked.contains(&i) {
                                    "[x]"
                                } else {
                                    "[ ]"
                                };
                                let style = if i == app.goal_picker_selected {
                                    Style::default().fg(theme::base()).bg(theme::green())
                                } else {
                                    Style::default().fg(theme::text())
                                };
                                ratatui::widgets::ListItem::new(Line::styled(
                                    format!("  {} #{} {} [{}]", checked, t.number, t.title, t.state),
                                    style,
                                ))
                            })
                            .collect();
                        f.render_widget(ratatui::widgets::List::new(items), inner);
                    }
                }
                Modal::Promote => {
                    f.render_widget(
                        widgets::popup::InputPopup {
                            title: "Promote to Task (enter title)",
                            text: &app.input_text,
                            cursor: app.input_cursor,
                        },
                        size,
                    );
                }
                Modal::None => {}
            }

            // Help overlay
            if app.show_help {
                let help_area = widgets::popup::centered_rect(60, 70, size);
                Clear.render(help_area, f.buffer_mut());
                let help_text = vec![
                    Line::from(""),
                    Line::styled(
                        "                Projects TUI — Help",
                        Style::default().fg(theme::mauve()),
                    ),
                    Line::from(""),
                    Line::styled("  Navigation", Style::default().fg(theme::blue())),
                    Line::styled(
                        "    j / ↓         Move down in list",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    k / ↑         Move up in list",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    Tab           Switch focus between panels",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    1-9, 0        Switch view (Tasks/Agents/Knowledge/Decisions/",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "                  Playbooks/Goals/Obs/Team/Int/Audit)  l = Logs",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    Enter         Expand selected item",
                        Style::default().fg(theme::text()),
                    ),
                    Line::from(""),
                    Line::styled("  Actions", Style::default().fg(theme::blue())),
                    Line::styled(
                        "    t             Transition selected task state",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    r             Reply to selected task / blocker",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    c             Post a comment on selected task",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    n             Create new task",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    e             Edit task (Tasks) / playbook / toggle int",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    a             Claim/assign task (Tasks) / accept / access",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    R             Release task from agent (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    i             Delegate task to agent/role (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    v             Toggle bulk select (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    b             Bulk transition selected tasks",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    d             Bulk delete selected tasks (in bulk mode)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    F             Toggle changed files (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    G             Refresh git status (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    /             Search / filter",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    ?             Toggle this help",
                        Style::default().fg(theme::text()),
                    ),
                    Line::from(""),
                    Line::styled("  Goals / Observations", Style::default().fg(theme::blue())),
                    Line::styled(
                        "    s             Change status (Goals/Observations)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    g             Link task to goal (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    l             Link tasks to goal (Goals view, multi-select)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    d             Dismiss observation (Observations view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    p             Promote observation to task (Observations view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::from(""),
                    Line::styled(
                        "  Decisions / Knowledge",
                        Style::default().fg(theme::blue()),
                    ),
                    Line::styled(
                        "    a             Accept decision / Agent access (Int)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    x             Reject decision (Decisions view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    S             Supersede decision (Decisions view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    X             Deprecate decision (Decisions view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    D             Delete entry (Decisions/Knowledge/Playbooks/Team/Int)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::from(""),
                    Line::styled(
                        "  Dependencies / Playbooks / Integrations / Audit",
                        Style::default().fg(theme::blue()),
                    ),
                    Line::styled(
                        "    w             Add task dependency (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    W             Remove task dependency (Tasks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    n             New playbook (Playbooks view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    e             Edit playbook / Toggle integration",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    h             Entity history (Audit view)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::from(""),
                    Line::styled("  Logs", Style::default().fg(theme::blue())),
                    Line::styled(
                        "    Enter         Edit LogQL query + submit",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    /             Filter log output by text",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    T / Y         Decrease / increase time range",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    N / M         Decrease / increase result limit",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    B             Toggle direction (forward/backward)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    PgUp / PgDn   Fast scroll",
                        Style::default().fg(theme::text()),
                    ),
                    Line::from(""),
                    Line::styled("  General", Style::default().fg(theme::blue())),
                    Line::styled(
                        "    `             View picker (all views)",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        "    L             Toggle light/dark theme",
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled("    q             Quit", Style::default().fg(theme::text())),
                    Line::styled(
                        "    S             Project settings (edit properties & CLAUDE.md)",
                        Style::default().fg(theme::text()),
                    ),
                ];
                let help_block = Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::mauve()))
                    .style(Style::default().bg(theme::mantle()));
                let help_p = Paragraph::new(help_text).block(help_block);
                f.render_widget(help_p, help_area);
            }

            // Task edit form
            if let Some(ref form) = app.task_edit_form {
                let form_area = widgets::popup::centered_rect(60, 50, size);
                Clear.render(form_area, f.buffer_mut());
                let block = Block::default()
                    .title(" Edit Task ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::peach()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Kind
                        Constraint::Length(2), // Priority
                        Constraint::Min(3),    // Spec
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_field = |f: &mut Frame,
                                    area: Rect,
                                    label: &str,
                                    value: &str,
                                    active: bool,
                                    scroll: (u16, u16)| {
                    let style = if active {
                        Style::default().fg(theme::blue())
                    } else {
                        Style::default().fg(theme::subtext0())
                    };
                    let val_style = if active {
                        Style::default().fg(theme::text())
                    } else {
                        Style::default().fg(theme::overlay0())
                    };
                    let p = Paragraph::new(vec![Line::from(vec![
                        Span::styled(format!(" {} ", label), style),
                        Span::styled(value.to_string(), val_style),
                    ])])
                    .wrap(Wrap { trim: false })
                    .scroll(scroll);
                    f.render_widget(p, area);
                };

                // Title
                {
                    let cursor_text = if form.active_field == 0 {
                        let bp = form
                            .title
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.title.len());
                        if bp < form.title.len() {
                            format!("{}│{}", &form.title[..bp], &form.title[bp..])
                        } else {
                            format!("{}│", &form.title)
                        }
                    } else {
                        form.title.clone()
                    };
                    let area = field_chunks[0];
                    let avail_w = area.width as usize;
                    let chars_before = " Title: ".len() + form.cursor;
                    let cursor_line = if avail_w > 0 {
                        chars_before / avail_w
                    } else {
                        0
                    };
                    let max_line = (area.height as usize).saturating_sub(1);
                    let scroll_y = cursor_line.saturating_sub(max_line) as u16;
                    render_field(
                        f,
                        area,
                        "Title:",
                        &cursor_text,
                        form.active_field == 0,
                        (scroll_y, 0),
                    );
                }

                // Kind
                {
                    let kind = TASK_KINDS[form.kind_index];
                    let val = format!("◀ {} ▶", kind);
                    render_field(
                        f,
                        field_chunks[1],
                        "Kind:",
                        &val,
                        form.active_field == 1,
                        (0, 0),
                    );
                }

                // Priority
                {
                    let val = format!("◀ {} ▶", form.priority);
                    render_field(
                        f,
                        field_chunks[2],
                        "Priority:",
                        &val,
                        form.active_field == 2,
                        (0, 0),
                    );
                }

                // Spec (field 3)
                {
                    let cursor_text = if form.active_field == 3 {
                        let bp = form
                            .spec
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.spec.len());
                        if bp < form.spec.len() {
                            format!("{}│{}", &form.spec[..bp], &form.spec[bp..])
                        } else {
                            format!("{}│", &form.spec)
                        }
                    } else {
                        form.spec.clone()
                    };
                    let area = field_chunks[3];
                    let avail_w = area.width as usize;
                    let chars_before = " Spec: ".len() + form.cursor;
                    let cursor_line = if avail_w > 0 {
                        chars_before / avail_w
                    } else {
                        0
                    };
                    let max_line = (area.height as usize).saturating_sub(1);
                    let scroll_y = cursor_line.saturating_sub(max_line) as u16;
                    render_field(
                        f,
                        area,
                        "Spec:",
                        &cursor_text,
                        form.active_field == 3,
                        (scroll_y, 0),
                    );
                }

                // Footer hint
                let hint = Paragraph::new(Line::styled(
                    " Tab: next field | Enter: save | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[4]);
            }

            // Task creation form
            if let Some(ref form) = app.task_form {
                let form_area = widgets::popup::centered_rect(60, 50, size);
                Clear.render(form_area, f.buffer_mut());
                let block = Block::default()
                    .title(" New Task ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::mauve()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Kind
                        Constraint::Length(2), // Priority
                        Constraint::Length(2), // Playbook
                        Constraint::Min(3),    // Spec
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                // Helper to render a field label + value
                let render_field = |f: &mut Frame,
                                    area: Rect,
                                    label: &str,
                                    value: &str,
                                    active: bool,
                                    scroll: (u16, u16)| {
                    let style = if active {
                        Style::default().fg(theme::blue())
                    } else {
                        Style::default().fg(theme::subtext0())
                    };
                    let val_style = if active {
                        Style::default().fg(theme::text())
                    } else {
                        Style::default().fg(theme::overlay0())
                    };
                    let p = Paragraph::new(vec![Line::from(vec![
                        Span::styled(format!(" {} ", label), style),
                        Span::styled(value.to_string(), val_style),
                    ])])
                    .wrap(Wrap { trim: false })
                    .scroll(scroll);
                    f.render_widget(p, area);
                };

                // Title
                {
                    let cursor_text = if form.active_field == 0 {
                        let bp = form
                            .title
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.title.len());
                        if bp < form.title.len() {
                            format!("{}│{}", &form.title[..bp], &form.title[bp..])
                        } else {
                            format!("{}│", &form.title)
                        }
                    } else {
                        form.title.clone()
                    };
                    let area = field_chunks[0];
                    let avail_w = area.width as usize;
                    let chars_before = " Title: ".len() + form.cursor;
                    let cursor_line = if avail_w > 0 {
                        chars_before / avail_w
                    } else {
                        0
                    };
                    let max_line = (area.height as usize).saturating_sub(1);
                    let scroll_y = cursor_line.saturating_sub(max_line) as u16;
                    render_field(
                        f,
                        area,
                        "Title:",
                        &cursor_text,
                        form.active_field == 0,
                        (scroll_y, 0),
                    );
                }

                // Kind
                {
                    let kind = TASK_KINDS[form.kind_index];
                    let val = format!("◀ {} ▶", kind);
                    render_field(
                        f,
                        field_chunks[1],
                        "Kind:",
                        &val,
                        form.active_field == 1,
                        (0, 0),
                    );
                }

                // Priority
                {
                    let val = format!("◀ {} ▶", form.priority);
                    render_field(
                        f,
                        field_chunks[2],
                        "Priority:",
                        &val,
                        form.active_field == 2,
                        (0, 0),
                    );
                }

                // Playbook (field 3)
                {
                    let val = if form.playbook_index == 0 {
                        "◀ None (manual) ▶".to_string()
                    } else if let Some(pb) = app.playbooks.get(form.playbook_index - 1) {
                        format!("◀ {} ▶", pb.title)
                    } else {
                        "◀ None (manual) ▶".to_string()
                    };
                    render_field(
                        f,
                        field_chunks[3],
                        "Playbook:",
                        &val,
                        form.active_field == 3,
                        (0, 0),
                    );
                }

                // Spec (field 4)
                {
                    let cursor_text = if form.active_field == 4 {
                        let bp = form
                            .spec
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.spec.len());
                        if bp < form.spec.len() {
                            format!("{}│{}", &form.spec[..bp], &form.spec[bp..])
                        } else {
                            format!("{}│", &form.spec)
                        }
                    } else {
                        form.spec.clone()
                    };
                    let area = field_chunks[4];
                    let avail_w = area.width as usize;
                    let chars_before = " Spec: ".len() + form.cursor;
                    let cursor_line = if avail_w > 0 {
                        chars_before / avail_w
                    } else {
                        0
                    };
                    let max_line = (area.height as usize).saturating_sub(1);
                    let scroll_y = cursor_line.saturating_sub(max_line) as u16;
                    render_field(
                        f,
                        area,
                        "Spec:",
                        &cursor_text,
                        form.active_field == 4,
                        (scroll_y, 0),
                    );
                }

                // Footer hint
                let hint = Paragraph::new(Line::styled(
                    " Tab: next field | Enter: submit | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[5]);
            }

            // Playbook creation/edit form
            if let Some(ref form) = app.playbook_form {
                let form_area = widgets::popup::centered_rect(80, 80, size);
                Clear.render(form_area, f.buffer_mut());
                let title = if form.editing_id.is_some() {
                    " Edit Playbook "
                } else {
                    " New Playbook "
                };
                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::peach()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Trigger
                        Constraint::Length(2), // Tags
                        Constraint::Length(1), // Steps header
                        Constraint::Min(4),    // Steps list / step editor
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_pb_field =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])]);
                        f.render_widget(p, area);
                    };

                // Helper to render text with cursor
                let text_with_cursor = |text: &str, cursor: usize, active: bool| -> String {
                    if active {
                        let bp = text
                            .char_indices()
                            .nth(cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(text.len());
                        if bp < text.len() {
                            format!("{}│{}", &text[..bp], &text[bp..])
                        } else {
                            format!("{}│", text)
                        }
                    } else {
                        text.to_string()
                    }
                };

                // Title
                {
                    let val = text_with_cursor(
                        &form.title,
                        form.cursor,
                        form.active_field == 0 && !form.editing_step,
                    );
                    render_pb_field(
                        f,
                        field_chunks[0],
                        "Title:",
                        &val,
                        form.active_field == 0 && !form.editing_step,
                    );
                }
                // Trigger
                {
                    let val = text_with_cursor(
                        &form.trigger,
                        form.cursor,
                        form.active_field == 1 && !form.editing_step,
                    );
                    render_pb_field(
                        f,
                        field_chunks[1],
                        "Trigger:",
                        &val,
                        form.active_field == 1 && !form.editing_step,
                    );
                }
                // Tags
                {
                    let val = text_with_cursor(
                        &form.tags,
                        form.cursor,
                        form.active_field == 2 && !form.editing_step,
                    );
                    render_pb_field(
                        f,
                        field_chunks[2],
                        "Tags:",
                        &val,
                        form.active_field == 2 && !form.editing_step,
                    );
                }

                // Steps header
                {
                    let style = if form.active_field == 3 {
                        Style::default().fg(theme::blue())
                    } else {
                        Style::default().fg(theme::subtext0())
                    };
                    let p = Paragraph::new(Line::styled(
                        format!(" Steps ({}):", form.steps.len()),
                        style,
                    ));
                    f.render_widget(p, field_chunks[3]);
                }

                // Steps area
                if form.active_field == 3 && form.editing_step {
                    // Step detail editor
                    if let Some(step) = form.steps.get(form.selected_step) {
                        let step_chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(1), // Step name
                                Constraint::Length(1), // Description
                                Constraint::Length(1), // on_complete
                                Constraint::Length(1), // timeout
                                Constraint::Length(1), // model
                                Constraint::Min(0),    // spacer
                            ])
                            .split(field_chunks[4]);

                        let step_label = format!(" Editing step {}:", form.selected_step);
                        let name_val =
                            text_with_cursor(&step.name, form.cursor, form.step_field == 0);
                        let desc_val = text_with_cursor(
                            &step.description,
                            form.cursor,
                            form.step_field == 1,
                        );
                        let on_complete =
                            app::ON_COMPLETE_OPTIONS[step.on_complete_index];
                        let timeout_val = text_with_cursor(
                            &step.timeout_minutes,
                            form.cursor,
                            form.step_field == 3,
                        );
                        let model_val = app::STEP_MODEL_OPTIONS[step.model_index];
                        let model_display = if model_val.is_empty() {
                            "(default)"
                        } else {
                            model_val
                        };

                        // Render each step field
                        let fields: Vec<(&str, String, bool)> = vec![
                            ("  Name:", name_val, form.step_field == 0),
                            ("  Desc:", desc_val, form.step_field == 1),
                            (
                                "  OnComplete:",
                                format!("◄ {} ►", on_complete),
                                form.step_field == 2,
                            ),
                            ("  Timeout:", timeout_val, form.step_field == 3),
                            (
                                "  Model:",
                                format!("◄ {} ►", model_display),
                                form.step_field == 4,
                            ),
                        ];

                        // Step header
                        let header_p = Paragraph::new(Line::styled(
                            step_label,
                            Style::default().fg(theme::green()),
                        ));
                        f.render_widget(header_p, step_chunks[0]);

                        for (idx, (label, val, active)) in fields.iter().enumerate() {
                            if idx + 1 < step_chunks.len() {
                                let style = if *active {
                                    Style::default().fg(theme::blue())
                                } else {
                                    Style::default().fg(theme::subtext0())
                                };
                                let val_style = if *active {
                                    Style::default().fg(theme::text())
                                } else {
                                    Style::default().fg(theme::overlay0())
                                };
                                let p = Paragraph::new(vec![Line::from(vec![
                                    Span::styled(label.to_string(), style),
                                    Span::raw(" "),
                                    Span::styled(val.clone(), val_style),
                                ])]);
                                // step_chunks[0] = header, [1..5] = fields
                                f.render_widget(p, step_chunks[idx + 1]);
                            }
                        }
                    }
                } else {
                    // Step list view
                    let mut lines: Vec<Line> = Vec::new();
                    if form.steps.is_empty() {
                        lines.push(Line::styled(
                            "  (no steps — press 'a' to add)",
                            Style::default().fg(theme::overlay0()),
                        ));
                    } else {
                        for (i, step) in form.steps.iter().enumerate() {
                            let selected = form.active_field == 3 && i == form.selected_step;
                            let on_complete =
                                app::ON_COMPLETE_OPTIONS[step.on_complete_index];
                            let model_str = app::STEP_MODEL_OPTIONS[step.model_index];
                            let model_display = if model_str.is_empty() {
                                String::new()
                            } else {
                                format!(" [{}]", model_str)
                            };
                            let timeout_display = if step.timeout_minutes.is_empty() {
                                String::new()
                            } else {
                                format!(" ({}m)", step.timeout_minutes)
                            };
                            let label = format!(
                                "  {}. {}{}{} → {}",
                                i, step.name, timeout_display, model_display, on_complete
                            );
                            let style = if selected {
                                Style::default().fg(theme::base()).bg(theme::peach())
                            } else {
                                Style::default().fg(theme::text())
                            };
                            lines.push(Line::styled(label, style));
                        }
                    }
                    let p = Paragraph::new(lines);
                    f.render_widget(p, field_chunks[4]);
                }

                // Footer hint
                let hint_text = if form.active_field == 3 && form.editing_step {
                    " Tab: next field | ◄►: cycle option | Esc: back to list"
                } else if form.active_field == 3 {
                    " ↑↓: select | Enter: edit | a: add | d: del | Ctrl+↑↓: reorder | Tab: next | Esc: cancel"
                } else {
                    " Tab: next field | Enter: submit | Esc: cancel"
                };
                let hint = Paragraph::new(Line::styled(
                    hint_text,
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[5]);
            }

            // Goal creation/edit form
            if let Some(ref form) = app.goal_form {
                let form_area = widgets::popup::centered_rect(60, 60, size);
                Clear.render(form_area, f.buffer_mut());
                let title = if form.editing_id.is_some() {
                    " Edit Goal "
                } else {
                    " New Goal "
                };
                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::green()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Description
                        Constraint::Length(2), // Success criteria
                        Constraint::Length(2), // Target date
                        Constraint::Length(2), // Status
                        Constraint::Length(2), // Goal type
                        Constraint::Length(2), // Priority
                        Constraint::Length(2), // Auto status
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_gf =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])])
                        .wrap(Wrap { trim: false });
                        f.render_widget(p, area);
                    };

                // Title with cursor
                {
                    let val = if form.active_field == 0 {
                        let bp = form
                            .title
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.title.len());
                        if bp < form.title.len() {
                            format!("{}│{}", &form.title[..bp], &form.title[bp..])
                        } else {
                            format!("{}│", &form.title)
                        }
                    } else {
                        form.title.clone()
                    };
                    render_gf(f, field_chunks[0], "Title:", &val, form.active_field == 0);
                }
                // Description with cursor
                {
                    let val = if form.active_field == 1 {
                        let bp = form
                            .description
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.description.len());
                        if bp < form.description.len() {
                            format!("{}│{}", &form.description[..bp], &form.description[bp..])
                        } else {
                            format!("{}│", &form.description)
                        }
                    } else {
                        form.description.clone()
                    };
                    render_gf(
                        f,
                        field_chunks[1],
                        "Description:",
                        &val,
                        form.active_field == 1,
                    );
                }
                // Success criteria with cursor
                {
                    let val = if form.active_field == 2 {
                        let bp = form
                            .success_criteria
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.success_criteria.len());
                        if bp < form.success_criteria.len() {
                            format!(
                                "{}│{}",
                                &form.success_criteria[..bp],
                                &form.success_criteria[bp..]
                            )
                        } else {
                            format!("{}│", &form.success_criteria)
                        }
                    } else {
                        form.success_criteria.clone()
                    };
                    render_gf(
                        f,
                        field_chunks[2],
                        "Criteria:",
                        &val,
                        form.active_field == 2,
                    );
                }
                // Target date with cursor
                {
                    let val = if form.active_field == 3 {
                        let bp = form
                            .target_date
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.target_date.len());
                        if bp < form.target_date.len() {
                            format!("{}│{}", &form.target_date[..bp], &form.target_date[bp..])
                        } else {
                            format!("{}│", &form.target_date)
                        }
                    } else {
                        form.target_date.clone()
                    };
                    render_gf(f, field_chunks[3], "Date:", &val, form.active_field == 3);
                }
                // Status selector
                {
                    let val = format!("◀ {} ▶", GOAL_STATUSES[form.status_index]);
                    render_gf(f, field_chunks[4], "Status:", &val, form.active_field == 4);
                }
                // Goal type selector
                {
                    let val = format!("◀ {} ▶", GOAL_TYPES[form.goal_type_index]);
                    render_gf(f, field_chunks[5], "Type:", &val, form.active_field == 5);
                }
                // Priority with cursor
                {
                    let val = if form.active_field == 6 {
                        let bp = form
                            .priority
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.priority.len());
                        if bp < form.priority.len() {
                            format!("{}│{}", &form.priority[..bp], &form.priority[bp..])
                        } else {
                            format!("{}│", &form.priority)
                        }
                    } else {
                        form.priority.clone()
                    };
                    render_gf(
                        f,
                        field_chunks[6],
                        "Priority:",
                        &val,
                        form.active_field == 6,
                    );
                }
                // Auto status toggle
                {
                    let val = if form.auto_status { "ON" } else { "OFF" };
                    render_gf(
                        f,
                        field_chunks[7],
                        "Auto Status:",
                        val,
                        form.active_field == 7,
                    );
                }
                let hint = Paragraph::new(Line::styled(
                    " Tab: next | Enter: submit | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[8]);
            }

            // Observation creation form
            if let Some(ref form) = app.observation_form {
                let form_area = widgets::popup::centered_rect(60, 40, size);
                Clear.render(form_area, f.buffer_mut());
                let block = Block::default()
                    .title(" New Observation ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::yellow()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Kind
                        Constraint::Length(2), // Severity
                        Constraint::Min(3),    // Description
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_of =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])])
                        .wrap(Wrap { trim: false });
                        f.render_widget(p, area);
                    };

                // Title
                {
                    let val = if form.active_field == 0 {
                        let bp = form
                            .title
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.title.len());
                        if bp < form.title.len() {
                            format!("{}│{}", &form.title[..bp], &form.title[bp..])
                        } else {
                            format!("{}│", &form.title)
                        }
                    } else {
                        form.title.clone()
                    };
                    render_of(f, field_chunks[0], "Title:", &val, form.active_field == 0);
                }
                // Kind selector
                {
                    let val = format!("◀ {} ▶", OBSERVATION_KINDS[form.kind_index]);
                    render_of(f, field_chunks[1], "Kind:", &val, form.active_field == 1);
                }
                // Severity selector
                {
                    let val = format!("◀ {} ▶", OBSERVATION_SEVERITIES[form.severity_index]);
                    render_of(
                        f,
                        field_chunks[2],
                        "Severity:",
                        &val,
                        form.active_field == 2,
                    );
                }
                // Description
                {
                    let val = if form.active_field == 3 {
                        let bp = form
                            .description
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.description.len());
                        if bp < form.description.len() {
                            format!("{}│{}", &form.description[..bp], &form.description[bp..])
                        } else {
                            format!("{}│", &form.description)
                        }
                    } else {
                        form.description.clone()
                    };
                    render_of(f, field_chunks[3], "Desc:", &val, form.active_field == 3);
                }
                let hint = Paragraph::new(Line::styled(
                    " Tab: next | Enter: submit | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[4]);
            }

            // Decision creation form
            if let Some(ref form) = app.decision_form {
                let form_area = widgets::popup::centered_rect(65, 55, size);
                Clear.render(form_area, f.buffer_mut());
                let block = Block::default()
                    .title(" New Decision ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::peach()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Context
                        Constraint::Length(2), // Decision
                        Constraint::Length(2), // Rationale
                        Constraint::Min(3),    // Alternatives
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_df =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])])
                        .wrap(Wrap { trim: false });
                        f.render_widget(p, area);
                    };

                let fields: Vec<(&str, &str, &str)> = vec![
                    ("Title:", &form.title, "title"),
                    ("Context:", &form.context, "context"),
                    ("Decision:", &form.decision, "decision"),
                    ("Rationale:", &form.rationale, "rationale"),
                    ("Alts:", &form.alternatives, "alternatives"),
                ];
                for (i, (label, text, _)) in fields.iter().enumerate() {
                    let val = if form.active_field == i {
                        let bp = text
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(idx, _)| idx)
                            .unwrap_or(text.len());
                        if bp < text.len() {
                            format!("{}│{}", &text[..bp], &text[bp..])
                        } else {
                            format!("{}│", text)
                        }
                    } else {
                        text.to_string()
                    };
                    render_df(f, field_chunks[i], label, &val, form.active_field == i);
                }
                let hint = Paragraph::new(Line::styled(
                    " Tab: next | Enter: submit | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[5]);
            }

            // Knowledge creation/edit form
            if let Some(ref form) = app.knowledge_form {
                let form_area = widgets::popup::centered_rect(60, 45, size);
                Clear.render(form_area, f.buffer_mut());
                let title = if form.editing_id.is_some() {
                    " Edit Knowledge "
                } else {
                    " New Knowledge "
                };
                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::teal()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Title
                        Constraint::Length(2), // Category
                        Constraint::Min(3),    // Content
                        Constraint::Length(2), // Tags
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_kf =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])])
                        .wrap(Wrap { trim: false });
                        f.render_widget(p, area);
                    };

                // Title
                {
                    let val = if form.active_field == 0 {
                        let bp = form
                            .title
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.title.len());
                        if bp < form.title.len() {
                            format!("{}│{}", &form.title[..bp], &form.title[bp..])
                        } else {
                            format!("{}│", &form.title)
                        }
                    } else {
                        form.title.clone()
                    };
                    render_kf(f, field_chunks[0], "Title:", &val, form.active_field == 0);
                }
                // Category selector
                {
                    let val = format!("◀ {} ▶", KNOWLEDGE_CATEGORIES[form.category_index]);
                    render_kf(
                        f,
                        field_chunks[1],
                        "Category:",
                        &val,
                        form.active_field == 1,
                    );
                }
                // Content
                {
                    let val = if form.active_field == 2 {
                        let bp = form
                            .content
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.content.len());
                        if bp < form.content.len() {
                            format!("{}│{}", &form.content[..bp], &form.content[bp..])
                        } else {
                            format!("{}│", &form.content)
                        }
                    } else {
                        form.content.clone()
                    };
                    render_kf(f, field_chunks[2], "Content:", &val, form.active_field == 2);
                }
                // Tags
                {
                    let val = if form.active_field == 3 {
                        let bp = form
                            .tags
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.tags.len());
                        if bp < form.tags.len() {
                            format!("{}│{}", &form.tags[..bp], &form.tags[bp..])
                        } else {
                            format!("{}│", &form.tags)
                        }
                    } else {
                        form.tags.clone()
                    };
                    render_kf(f, field_chunks[3], "Tags:", &val, form.active_field == 3);
                }
                let hint = Paragraph::new(Line::styled(
                    " Tab: next | Enter: submit | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[4]);
            }

            // Integration creation form
            if let Some(ref form) = app.integration_form {
                let form_area = widgets::popup::centered_rect(60, 45, size);
                Clear.render(form_area, f.buffer_mut());
                let block = Block::default()
                    .title(" New Integration ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::blue()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Name
                        Constraint::Length(2), // Kind
                        Constraint::Length(2), // Provider
                        Constraint::Length(2), // Base URL
                        Constraint::Length(2), // Auth type
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_if =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])])
                        .wrap(Wrap { trim: false });
                        f.render_widget(p, area);
                    };

                // Name
                {
                    let val = if form.active_field == 0 {
                        let bp = form
                            .name
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.name.len());
                        if bp < form.name.len() {
                            format!("{}│{}", &form.name[..bp], &form.name[bp..])
                        } else {
                            format!("{}│", &form.name)
                        }
                    } else {
                        form.name.clone()
                    };
                    render_if(f, field_chunks[0], "Name:", &val, form.active_field == 0);
                }
                // Kind selector
                {
                    let val = format!("◀ {} ▶", INTEGRATION_KINDS[form.kind_index]);
                    render_if(f, field_chunks[1], "Kind:", &val, form.active_field == 1);
                }
                // Provider
                {
                    let val = if form.active_field == 2 {
                        let bp = form
                            .provider
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.provider.len());
                        if bp < form.provider.len() {
                            format!("{}│{}", &form.provider[..bp], &form.provider[bp..])
                        } else {
                            format!("{}│", &form.provider)
                        }
                    } else {
                        form.provider.clone()
                    };
                    render_if(
                        f,
                        field_chunks[2],
                        "Provider:",
                        &val,
                        form.active_field == 2,
                    );
                }
                // Base URL
                {
                    let val = if form.active_field == 3 {
                        let bp = form
                            .base_url
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.base_url.len());
                        if bp < form.base_url.len() {
                            format!("{}│{}", &form.base_url[..bp], &form.base_url[bp..])
                        } else {
                            format!("{}│", &form.base_url)
                        }
                    } else {
                        form.base_url.clone()
                    };
                    render_if(
                        f,
                        field_chunks[3],
                        "Base URL:",
                        &val,
                        form.active_field == 3,
                    );
                }
                // Auth type selector
                {
                    let val = format!("◀ {} ▶", INTEGRATION_AUTH_TYPES[form.auth_type_index]);
                    render_if(f, field_chunks[4], "Auth:", &val, form.active_field == 4);
                }
                let hint = Paragraph::new(Line::styled(
                    " Tab: next | Enter: submit | Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[5]);
            }

            // Verification creation/edit form
            if let Some(ref form) = app.verification_form {
                let form_area = widgets::popup::centered_rect(60, 60, size);
                Clear.render(form_area, f.buffer_mut());
                let title = if form.editing_id.is_some() {
                    " Edit Verification "
                } else {
                    " New Verification "
                };
                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme::mauve()))
                    .style(Style::default().bg(theme::mantle()));
                let inner = block.inner(form_area);
                f.render_widget(block, form_area);

                let field_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Task
                        Constraint::Length(2), // Kind
                        Constraint::Length(2), // Status
                        Constraint::Length(2), // Title
                        Constraint::Min(2),    // Detail
                        Constraint::Min(2),    // Evidence
                        Constraint::Length(1), // Footer
                    ])
                    .split(inner);

                let render_vf_field =
                    |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
                        let style = if active {
                            Style::default().fg(theme::blue())
                        } else {
                            Style::default().fg(theme::subtext0())
                        };
                        let val_style = if active {
                            Style::default().fg(theme::text())
                        } else {
                            Style::default().fg(theme::overlay0())
                        };
                        let p = Paragraph::new(vec![Line::from(vec![
                            Span::styled(format!(" {} ", label), style),
                            Span::styled(value.to_string(), val_style),
                        ])])
                        .wrap(Wrap { trim: false });
                        f.render_widget(p, area);
                    };

                // Task picker (field 0)
                {
                    let task_label = if app.tasks.is_empty() {
                        "◀ (no tasks) ▶".to_string()
                    } else {
                        let t = &app.tasks[form.task_index.min(app.tasks.len() - 1)];
                        format!("◀ {} ▶", t.title)
                    };
                    render_vf_field(
                        f,
                        field_chunks[0],
                        "Task:",
                        &task_label,
                        form.active_field == 0,
                    );
                }

                // Kind picker (field 1)
                {
                    let kind = VERIFICATION_KINDS[form.kind_index];
                    render_vf_field(
                        f,
                        field_chunks[1],
                        "Kind:",
                        &format!("◀ {} ▶", kind),
                        form.active_field == 1,
                    );
                }

                // Status picker (field 2)
                {
                    let status = VERIFICATION_STATUSES[form.status_index];
                    render_vf_field(
                        f,
                        field_chunks[2],
                        "Status:",
                        &format!("◀ {} ▶", status),
                        form.active_field == 2,
                    );
                }

                // Title text (field 3)
                {
                    let val = if form.active_field == 3 {
                        let bp = form
                            .title
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.title.len());
                        if bp < form.title.len() {
                            format!("{}│{}", &form.title[..bp], &form.title[bp..])
                        } else {
                            format!("{}│", &form.title)
                        }
                    } else {
                        form.title.clone()
                    };
                    render_vf_field(f, field_chunks[3], "Title:", &val, form.active_field == 3);
                }

                // Detail text (field 4)
                {
                    let val = if form.active_field == 4 {
                        let bp = form
                            .detail
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.detail.len());
                        if bp < form.detail.len() {
                            format!("{}│{}", &form.detail[..bp], &form.detail[bp..])
                        } else {
                            format!("{}│", &form.detail)
                        }
                    } else {
                        form.detail.clone()
                    };
                    render_vf_field(f, field_chunks[4], "Detail:", &val, form.active_field == 4);
                }

                // Evidence text (field 5)
                {
                    let val = if form.active_field == 5 {
                        let bp = form
                            .evidence
                            .char_indices()
                            .nth(form.cursor)
                            .map(|(i, _)| i)
                            .unwrap_or(form.evidence.len());
                        if bp < form.evidence.len() {
                            format!("{}│{}", &form.evidence[..bp], &form.evidence[bp..])
                        } else {
                            format!("{}│", &form.evidence)
                        }
                    } else {
                        form.evidence.clone()
                    };
                    render_vf_field(
                        f,
                        field_chunks[5],
                        "Evidence:",
                        &val,
                        form.active_field == 5,
                    );
                }

                // Footer hint
                let hint = Paragraph::new(Line::styled(
                    " Tab: next  ◀▶: cycle  Enter: submit  Esc: cancel",
                    Style::default().fg(theme::overlay0()),
                ));
                f.render_widget(hint, field_chunks[6]);
            }
        })?;

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Clear error flash on any keypress
                app.last_error = None;

                // Ctrl+C always quits
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                // Help overlay intercepts keys
                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => app.show_help = false,
                        _ => {}
                    }
                } else if app.task_edit_form.is_some() {
                    let form = app.task_edit_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.task_edit_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 4;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                3 => form.spec.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let task_id = form.task_id;
                                let title = form.title.clone();
                                let kind = TASK_KINDS[form.kind_index].to_string();
                                let priority = form.priority;
                                let spec = form.spec.clone();
                                let pid = app.current_project;
                                app.task_edit_form = None;
                                let api = api.clone();
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    match api
                                        .update_task(
                                            task_id,
                                            serde_json::json!({
                                                "title": title,
                                                "kind": kind,
                                                "priority": priority,
                                                "context": { "spec": spec }
                                            }),
                                        )
                                        .await
                                    {
                                        Ok(_) => {
                                            if let Some(pid) = pid {
                                                if let Ok(resp) = api.list_tasks(pid).await {
                                                    let _ = tx.send(ApiMsg::Tasks(resp.data)).await;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let _ = tx
                                                .send(ApiMsg::Error(format!("Edit task: {e}")))
                                                .await;
                                        }
                                    }
                                });
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            1 => {
                                if form.kind_index > 0 {
                                    form.kind_index -= 1;
                                } else {
                                    form.kind_index = TASK_KINDS.len() - 1;
                                }
                            }
                            2 => {
                                if form.priority > 1 {
                                    form.priority -= 1;
                                }
                            }
                            0 => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                            3 => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Right => match form.active_field {
                            1 => {
                                form.kind_index = (form.kind_index + 1) % TASK_KINDS.len();
                            }
                            2 => {
                                if form.priority < 5 {
                                    form.priority += 1;
                                }
                            }
                            0 => {
                                if form.cursor < form.title.chars().count() {
                                    form.cursor += 1;
                                }
                            }
                            3 => {
                                if form.cursor < form.spec.chars().count() {
                                    form.cursor += 1;
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Backspace => match form.active_field {
                            0 if form.cursor > 0 => {
                                form.cursor -= 1;
                                let bp = form
                                    .title
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.title.len());
                                form.title.remove(bp);
                            }
                            3 if form.cursor > 0 => {
                                form.cursor -= 1;
                                let bp = form
                                    .spec
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.spec.len());
                                form.spec.remove(bp);
                            }
                            _ => {}
                        },
                        KeyCode::Char(c) => match form.active_field {
                            0 => {
                                let bp = form
                                    .title
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.title.len());
                                form.title.insert(bp, c);
                                form.cursor += 1;
                            }
                            3 => {
                                let bp = form
                                    .spec
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.spec.len());
                                form.spec.insert(bp, c);
                                form.cursor += 1;
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                } else if app.task_form.is_some() {
                    let playbook_count = app.playbooks.len();
                    let form = app.task_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.task_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 5;
                            // Reset cursor to end of target text field
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                4 => form.spec.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let title = form.title.clone();
                                let kind = TASK_KINDS[form.kind_index].to_string();
                                let priority = form.priority;
                                let spec = form.spec.clone();
                                let playbook_id = if form.playbook_index > 0 {
                                    app.playbooks.get(form.playbook_index - 1).map(|pb| pb.id)
                                } else {
                                    None
                                };
                                let pid = app.current_project;
                                app.task_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        match api
                                            .create_task(
                                                pid,
                                                &title,
                                                &kind,
                                                priority,
                                                &spec,
                                                playbook_id,
                                            )
                                            .await
                                        {
                                            Ok(_) => {
                                                if let Ok(resp) = api.list_tasks(pid).await {
                                                    let _ = tx.send(ApiMsg::Tasks(resp.data)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!(
                                                        "Create task: {e}"
                                                    )))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            1 => {
                                if form.kind_index > 0 {
                                    form.kind_index -= 1;
                                } else {
                                    form.kind_index = TASK_KINDS.len() - 1;
                                }
                            }
                            2 => {
                                if form.priority > 1 {
                                    form.priority -= 1;
                                }
                            }
                            3 => {
                                // Playbook selector: 0=None, 1..=N=playbooks
                                if form.playbook_index > 0 {
                                    form.playbook_index -= 1;
                                } else {
                                    form.playbook_index = playbook_count;
                                }
                            }
                            0 => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                            4 => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Right => match form.active_field {
                            1 => {
                                form.kind_index = (form.kind_index + 1) % TASK_KINDS.len();
                            }
                            2 => {
                                if form.priority < 5 {
                                    form.priority += 1;
                                }
                            }
                            3 => {
                                // Playbook selector
                                form.playbook_index =
                                    (form.playbook_index + 1) % (playbook_count + 1);
                            }
                            0 => {
                                if form.cursor < form.title.chars().count() {
                                    form.cursor += 1;
                                }
                            }
                            4 => {
                                if form.cursor < form.spec.chars().count() {
                                    form.cursor += 1;
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Backspace => match form.active_field {
                            0 if form.cursor > 0 => {
                                form.cursor -= 1;
                                let bp = form
                                    .title
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.title.len());
                                form.title.remove(bp);
                            }
                            4 if form.cursor > 0 => {
                                form.cursor -= 1;
                                let bp = form
                                    .spec
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.spec.len());
                                form.spec.remove(bp);
                            }
                            _ => {}
                        },
                        KeyCode::Char(c) => match form.active_field {
                            0 => {
                                let bp = form
                                    .title
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.title.len());
                                form.title.insert(bp, c);
                                form.cursor += 1;
                            }
                            4 => {
                                let bp = form
                                    .spec
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(form.spec.len());
                                form.spec.insert(bp, c);
                                form.cursor += 1;
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                } else if app.playbook_form.is_some() {
                    let form = app.playbook_form.as_mut().unwrap();
                    if form.active_field == 3 && form.editing_step {
                        // Step detail editing mode
                        match key.code {
                            KeyCode::Esc => {
                                form.editing_step = false;
                                form.cursor = 0;
                            }
                            KeyCode::Tab => {
                                form.step_field = (form.step_field + 1) % 5;
                                form.cursor = if let Some(step) = form.steps.get(form.selected_step)
                                {
                                    match form.step_field {
                                        0 => step.name.chars().count(),
                                        1 => step.description.chars().count(),
                                        3 => step.timeout_minutes.chars().count(),
                                        _ => 0,
                                    }
                                } else {
                                    0
                                };
                            }
                            KeyCode::Left => {
                                if let Some(step) = form.steps.get_mut(form.selected_step) {
                                    match form.step_field {
                                        2 => {
                                            // on_complete selector
                                            if step.on_complete_index > 0 {
                                                step.on_complete_index -= 1;
                                            } else {
                                                step.on_complete_index =
                                                    app::ON_COMPLETE_OPTIONS.len() - 1;
                                            }
                                        }
                                        4 => {
                                            // model selector
                                            if step.model_index > 0 {
                                                step.model_index -= 1;
                                            } else {
                                                step.model_index =
                                                    app::STEP_MODEL_OPTIONS.len() - 1;
                                            }
                                        }
                                        _ => {
                                            if form.cursor > 0 {
                                                form.cursor -= 1;
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Right => {
                                if let Some(step) = form.steps.get_mut(form.selected_step) {
                                    match form.step_field {
                                        2 => {
                                            step.on_complete_index = (step.on_complete_index + 1)
                                                % app::ON_COMPLETE_OPTIONS.len();
                                        }
                                        4 => {
                                            step.model_index = (step.model_index + 1)
                                                % app::STEP_MODEL_OPTIONS.len();
                                        }
                                        _ => {
                                            let len = if let Some(step) =
                                                form.steps.get(form.selected_step)
                                            {
                                                match form.step_field {
                                                    0 => step.name.chars().count(),
                                                    1 => step.description.chars().count(),
                                                    3 => step.timeout_minutes.chars().count(),
                                                    _ => 0,
                                                }
                                            } else {
                                                0
                                            };
                                            if form.cursor < len {
                                                form.cursor += 1;
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                if form.cursor > 0 {
                                    if let Some(step) = form.steps.get_mut(form.selected_step) {
                                        let text = match form.step_field {
                                            0 => &mut step.name,
                                            1 => &mut step.description,
                                            3 => &mut step.timeout_minutes,
                                            _ => continue,
                                        };
                                        form.cursor -= 1;
                                        let bp = text
                                            .char_indices()
                                            .nth(form.cursor)
                                            .map(|(i, _)| i)
                                            .unwrap_or(text.len());
                                        text.remove(bp);
                                    }
                                }
                            }
                            KeyCode::Char(c) => {
                                if let Some(step) = form.steps.get_mut(form.selected_step) {
                                    let text = match form.step_field {
                                        0 => &mut step.name,
                                        1 => &mut step.description,
                                        3 => &mut step.timeout_minutes,
                                        _ => continue,
                                    };
                                    // For timeout, only allow digits
                                    if form.step_field == 3 && !c.is_ascii_digit() {
                                        continue;
                                    }
                                    let bp = text
                                        .char_indices()
                                        .nth(form.cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(text.len());
                                    text.insert(bp, c);
                                    form.cursor += 1;
                                }
                            }
                            _ => {}
                        }
                    } else if form.active_field == 3 {
                        // Steps list mode
                        match key.code {
                            KeyCode::Esc => {
                                app.playbook_form = None;
                            }
                            KeyCode::Tab => {
                                form.active_field = 0;
                                form.cursor = form.title.chars().count();
                            }
                            KeyCode::BackTab => {
                                form.active_field = 2;
                                form.cursor = form.tags.chars().count();
                            }
                            KeyCode::Up => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    // Reorder: move step up
                                    if form.selected_step > 0 {
                                        form.steps.swap(form.selected_step, form.selected_step - 1);
                                        form.selected_step -= 1;
                                    }
                                } else if form.selected_step > 0 {
                                    form.selected_step -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    // Reorder: move step down
                                    if form.selected_step + 1 < form.steps.len() {
                                        form.steps.swap(form.selected_step, form.selected_step + 1);
                                        form.selected_step += 1;
                                    }
                                } else if form.selected_step + 1 < form.steps.len() {
                                    form.selected_step += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if !form.steps.is_empty() {
                                    form.editing_step = true;
                                    form.step_field = 0;
                                    form.cursor = form
                                        .steps
                                        .get(form.selected_step)
                                        .map(|s| s.name.chars().count())
                                        .unwrap_or(0);
                                }
                            }
                            KeyCode::Char('a') => {
                                // Add new step
                                form.steps.push(app::PlaybookStepForm::default());
                                form.selected_step = form.steps.len() - 1;
                                form.editing_step = true;
                                form.step_field = 0;
                                form.cursor = 0;
                            }
                            KeyCode::Char('d') => {
                                // Delete selected step
                                if !form.steps.is_empty() {
                                    form.steps.remove(form.selected_step);
                                    if form.selected_step >= form.steps.len()
                                        && !form.steps.is_empty()
                                    {
                                        form.selected_step = form.steps.len() - 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Header fields (title, trigger, tags)
                        match key.code {
                            KeyCode::Esc => {
                                app.playbook_form = None;
                            }
                            KeyCode::Tab => {
                                form.active_field = (form.active_field + 1) % 4;
                                form.cursor = match form.active_field {
                                    0 => form.title.chars().count(),
                                    1 => form.trigger.chars().count(),
                                    2 => form.tags.chars().count(),
                                    _ => 0,
                                };
                            }
                            KeyCode::BackTab => {
                                form.active_field = if form.active_field == 0 {
                                    3
                                } else {
                                    form.active_field - 1
                                };
                                form.cursor = match form.active_field {
                                    0 => form.title.chars().count(),
                                    1 => form.trigger.chars().count(),
                                    2 => form.tags.chars().count(),
                                    _ => 0,
                                };
                            }
                            KeyCode::Enter => {
                                // Submit form
                                if !form.title.is_empty() {
                                    let title = form.title.clone();
                                    let trigger = form.trigger.clone();
                                    let tags: Vec<String> = form
                                        .tags
                                        .split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    let steps_json = form.steps_to_json();
                                    let editing_id = form.editing_id;
                                    let pid = app.current_project;
                                    app.playbook_form = None;
                                    if let Some(pid) = pid {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            let result = if let Some(pb_id) = editing_id {
                                                api.update_playbook(
                                                    pb_id,
                                                    serde_json::json!({
                                                        "title": title,
                                                        "trigger_description": trigger,
                                                        "tags": tags,
                                                        "steps": steps_json,
                                                    }),
                                                )
                                                .await
                                                .map(|_| ())
                                            } else {
                                                api.create_playbook(
                                                    pid, &title, &trigger, steps_json, tags,
                                                )
                                                .await
                                                .map(|_| ())
                                            };
                                            match result {
                                                Ok(()) => {
                                                    if let Ok(pbs) = api.list_playbooks(pid).await {
                                                        let _ =
                                                            tx.send(ApiMsg::Playbooks(pbs)).await;
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Playbook: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Left => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                let len = match form.active_field {
                                    0 => form.title.chars().count(),
                                    1 => form.trigger.chars().count(),
                                    2 => form.tags.chars().count(),
                                    _ => 0,
                                };
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                            KeyCode::Backspace => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                    let text = match form.active_field {
                                        0 => &mut form.title,
                                        1 => &mut form.trigger,
                                        2 => &mut form.tags,
                                        _ => continue,
                                    };
                                    let bp = text
                                        .char_indices()
                                        .nth(form.cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(text.len());
                                    text.remove(bp);
                                }
                            }
                            KeyCode::Char(c) => {
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    1 => &mut form.trigger,
                                    2 => &mut form.tags,
                                    _ => continue,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                            _ => {}
                        }
                    }
                } else if app.goal_form.is_some() {
                    let form = app.goal_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.goal_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 8;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                1 => form.description.chars().count(),
                                2 => form.success_criteria.chars().count(),
                                3 => form.target_date.chars().count(),
                                6 => form.priority.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let title = form.title.clone();
                                let description = form.description.clone();
                                let success_criteria = form.success_criteria.clone();
                                let target_date = form.target_date.clone();
                                let status = GOAL_STATUSES[form.status_index].to_string();
                                let goal_type = GOAL_TYPES[form.goal_type_index].to_string();
                                let priority: i32 = form.priority.parse().unwrap_or(0);
                                let auto_status = form.auto_status;
                                let editing_id = form.editing_id;
                                let pid = app.current_project;
                                app.goal_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let mut body = serde_json::json!({
                                            "title": title,
                                            "description": description,
                                            "status": status,
                                            "goal_type": goal_type,
                                            "priority": priority,
                                            "auto_status": auto_status,
                                        });
                                        if !success_criteria.is_empty() {
                                            body["success_criteria"] =
                                                serde_json::json!([success_criteria]);
                                        }
                                        if !target_date.is_empty() {
                                            body["target_date"] = serde_json::json!(target_date);
                                        }
                                        let result = if let Some(gid) = editing_id {
                                            api.update_goal(gid, body).await.map(|_| ())
                                        } else {
                                            match api
                                                .create_goal(
                                                    pid,
                                                    &title,
                                                    &description,
                                                    &goal_type,
                                                    priority,
                                                    None,
                                                    auto_status,
                                                )
                                                .await
                                            {
                                                Ok(g) => {
                                                    if !success_criteria.is_empty()
                                                        || !target_date.is_empty()
                                                        || status != "active"
                                                    {
                                                        let mut upd = serde_json::json!({});
                                                        if !success_criteria.is_empty() {
                                                            upd["success_criteria"] =
                                                                serde_json::json!([
                                                                    success_criteria
                                                                ]);
                                                        }
                                                        if !target_date.is_empty() {
                                                            upd["target_date"] =
                                                                serde_json::json!(target_date);
                                                        }
                                                        if status != "active" {
                                                            upd["status"] =
                                                                serde_json::json!(status);
                                                        }
                                                        api.update_goal(g.id, upd).await.map(|_| ())
                                                    } else {
                                                        Ok(())
                                                    }
                                                }
                                                Err(e) => Err(e),
                                            }
                                        };
                                        match result {
                                            Ok(()) => {
                                                if let Ok(goals) = api.list_goals(pid).await {
                                                    let _ = tx.send(ApiMsg::Goals(goals)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Goal: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            4 => {
                                if form.status_index > 0 {
                                    form.status_index -= 1;
                                } else {
                                    form.status_index = GOAL_STATUSES.len() - 1;
                                }
                            }
                            5 => {
                                if form.goal_type_index > 0 {
                                    form.goal_type_index -= 1;
                                } else {
                                    form.goal_type_index = GOAL_TYPES.len() - 1;
                                }
                            }
                            7 => {
                                form.auto_status = !form.auto_status;
                            }
                            _ => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                        },
                        KeyCode::Right => match form.active_field {
                            4 => {
                                form.status_index = (form.status_index + 1) % GOAL_STATUSES.len();
                            }
                            5 => {
                                form.goal_type_index =
                                    (form.goal_type_index + 1) % GOAL_TYPES.len();
                            }
                            7 => {
                                form.auto_status = !form.auto_status;
                            }
                            _ => {
                                let len = match form.active_field {
                                    0 => form.title.chars().count(),
                                    1 => form.description.chars().count(),
                                    2 => form.success_criteria.chars().count(),
                                    3 => form.target_date.chars().count(),
                                    6 => form.priority.chars().count(),
                                    _ => 0,
                                };
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                        },
                        KeyCode::Backspace => {
                            if form.active_field == 4
                                || form.active_field == 5
                                || form.active_field == 7
                            {
                                // selectors/toggle, ignore backspace
                            } else if form.cursor > 0 {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    1 => &mut form.description,
                                    2 => &mut form.success_criteria,
                                    3 => &mut form.target_date,
                                    6 => &mut form.priority,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if form.active_field == 4
                                || form.active_field == 5
                                || form.active_field == 7
                            {
                                // selectors/toggle, ignore chars
                            } else {
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    1 => &mut form.description,
                                    2 => &mut form.success_criteria,
                                    3 => &mut form.target_date,
                                    6 => &mut form.priority,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.observation_form.is_some() {
                    let form = app.observation_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.observation_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 4;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                3 => form.description.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let title = form.title.clone();
                                let description = form.description.clone();
                                let kind = OBSERVATION_KINDS[form.kind_index].to_string();
                                let severity =
                                    OBSERVATION_SEVERITIES[form.severity_index].to_string();
                                let pid = app.current_project;
                                app.observation_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let body = serde_json::json!({
                                            "kind": kind,
                                            "severity": severity,
                                            "title": title,
                                            "description": description,
                                        });
                                        match api.create_observation(pid, body).await {
                                            Ok(_) => {
                                                if let Ok(obs) = api.list_observations(pid).await {
                                                    let _ =
                                                        tx.send(ApiMsg::Observations(obs)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!(
                                                        "Observation: {e}"
                                                    )))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            1 => {
                                if form.kind_index > 0 {
                                    form.kind_index -= 1;
                                } else {
                                    form.kind_index = OBSERVATION_KINDS.len() - 1;
                                }
                            }
                            2 => {
                                if form.severity_index > 0 {
                                    form.severity_index -= 1;
                                } else {
                                    form.severity_index = OBSERVATION_SEVERITIES.len() - 1;
                                }
                            }
                            _ => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                        },
                        KeyCode::Right => match form.active_field {
                            1 => {
                                form.kind_index = (form.kind_index + 1) % OBSERVATION_KINDS.len();
                            }
                            2 => {
                                form.severity_index =
                                    (form.severity_index + 1) % OBSERVATION_SEVERITIES.len();
                            }
                            _ => {
                                let len = match form.active_field {
                                    0 => form.title.chars().count(),
                                    3 => form.description.chars().count(),
                                    _ => 0,
                                };
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                        },
                        KeyCode::Backspace => {
                            if form.cursor > 0 && matches!(form.active_field, 0 | 3) {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    3 => &mut form.description,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if matches!(form.active_field, 0 | 3) {
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    3 => &mut form.description,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.decision_form.is_some() {
                    let form = app.decision_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.decision_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 5;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                1 => form.context.chars().count(),
                                2 => form.decision.chars().count(),
                                3 => form.rationale.chars().count(),
                                4 => form.alternatives.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let title = form.title.clone();
                                let context = form.context.clone();
                                let decision = form.decision.clone();
                                let rationale = form.rationale.clone();
                                let alternatives = form.alternatives.clone();
                                let pid = app.current_project;
                                app.decision_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let alts: Vec<serde_json::Value> = alternatives
                                            .lines()
                                            .filter(|l| !l.trim().is_empty())
                                            .map(|l| serde_json::json!({"name": l.trim()}))
                                            .collect();
                                        let body = serde_json::json!({
                                            "title": title,
                                            "context": context,
                                            "decision": decision,
                                            "rationale": rationale,
                                            "alternatives": alts,
                                        });
                                        match api.create_decision(pid, body).await {
                                            Ok(_) => {
                                                if let Ok(decs) = api.list_decisions(pid).await {
                                                    let _ = tx.send(ApiMsg::Decisions(decs)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Decision: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => {
                            if form.cursor > 0 {
                                form.cursor -= 1;
                            }
                        }
                        KeyCode::Right => {
                            let len = match form.active_field {
                                0 => form.title.chars().count(),
                                1 => form.context.chars().count(),
                                2 => form.decision.chars().count(),
                                3 => form.rationale.chars().count(),
                                4 => form.alternatives.chars().count(),
                                _ => 0,
                            };
                            if form.cursor < len {
                                form.cursor += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if form.cursor > 0 {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    1 => &mut form.context,
                                    2 => &mut form.decision,
                                    3 => &mut form.rationale,
                                    4 => &mut form.alternatives,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            let text = match form.active_field {
                                0 => &mut form.title,
                                1 => &mut form.context,
                                2 => &mut form.decision,
                                3 => &mut form.rationale,
                                4 => &mut form.alternatives,
                                _ => &mut form.title,
                            };
                            let bp = text
                                .char_indices()
                                .nth(form.cursor)
                                .map(|(i, _)| i)
                                .unwrap_or(text.len());
                            text.insert(bp, c);
                            form.cursor += 1;
                        }
                        _ => {}
                    }
                } else if app.knowledge_form.is_some() {
                    let form = app.knowledge_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.knowledge_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 4;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                2 => form.content.chars().count(),
                                3 => form.tags.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let title = form.title.clone();
                                let category =
                                    KNOWLEDGE_CATEGORIES[form.category_index].to_string();
                                let content = form.content.clone();
                                let tags: Vec<String> = form
                                    .tags
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                let editing_id = form.editing_id;
                                let pid = app.current_project;
                                app.knowledge_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let body = serde_json::json!({
                                            "title": title,
                                            "category": category,
                                            "content": content,
                                            "tags": tags,
                                        });
                                        let result = if let Some(kid) = editing_id {
                                            api.update_knowledge(kid, body).await.map(|_| ())
                                        } else {
                                            api.create_knowledge(pid, body).await.map(|_| ())
                                        };
                                        match result {
                                            Ok(()) => {
                                                if let Ok(k) = api.list_knowledge(pid).await {
                                                    let _ = tx.send(ApiMsg::Knowledge(k)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Knowledge: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            1 => {
                                if form.category_index > 0 {
                                    form.category_index -= 1;
                                } else {
                                    form.category_index = KNOWLEDGE_CATEGORIES.len() - 1;
                                }
                            }
                            _ => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                        },
                        KeyCode::Right => match form.active_field {
                            1 => {
                                form.category_index =
                                    (form.category_index + 1) % KNOWLEDGE_CATEGORIES.len();
                            }
                            _ => {
                                let len = match form.active_field {
                                    0 => form.title.chars().count(),
                                    2 => form.content.chars().count(),
                                    3 => form.tags.chars().count(),
                                    _ => 0,
                                };
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                        },
                        KeyCode::Backspace => {
                            if form.cursor > 0 && matches!(form.active_field, 0 | 2 | 3) {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    2 => &mut form.content,
                                    3 => &mut form.tags,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if matches!(form.active_field, 0 | 2 | 3) {
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    2 => &mut form.content,
                                    3 => &mut form.tags,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.integration_form.is_some() {
                    let form = app.integration_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.integration_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 5;
                            form.cursor = match form.active_field {
                                0 => form.name.chars().count(),
                                2 => form.provider.chars().count(),
                                3 => form.base_url.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.name.is_empty() {
                                let name = form.name.clone();
                                let kind = INTEGRATION_KINDS[form.kind_index].to_string();
                                let provider = form.provider.clone();
                                let base_url = form.base_url.clone();
                                let auth_type =
                                    INTEGRATION_AUTH_TYPES[form.auth_type_index].to_string();
                                let pid = app.current_project;
                                app.integration_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let mut body = serde_json::json!({
                                            "name": name,
                                            "kind": kind,
                                            "provider": provider,
                                            "auth_type": auth_type,
                                            "enabled": true,
                                        });
                                        if !base_url.is_empty() {
                                            body["base_url"] = serde_json::json!(base_url);
                                        }
                                        match api.create_integration(pid, body).await {
                                            Ok(_) => {
                                                if let Ok(ints) = api.list_integrations(pid).await {
                                                    let _ =
                                                        tx.send(ApiMsg::Integrations(ints)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!(
                                                        "Integration: {e}"
                                                    )))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            1 => {
                                if form.kind_index > 0 {
                                    form.kind_index -= 1;
                                } else {
                                    form.kind_index = INTEGRATION_KINDS.len() - 1;
                                }
                            }
                            4 => {
                                if form.auth_type_index > 0 {
                                    form.auth_type_index -= 1;
                                } else {
                                    form.auth_type_index = INTEGRATION_AUTH_TYPES.len() - 1;
                                }
                            }
                            _ => {
                                if form.cursor > 0 {
                                    form.cursor -= 1;
                                }
                            }
                        },
                        KeyCode::Right => match form.active_field {
                            1 => {
                                form.kind_index = (form.kind_index + 1) % INTEGRATION_KINDS.len();
                            }
                            4 => {
                                form.auth_type_index =
                                    (form.auth_type_index + 1) % INTEGRATION_AUTH_TYPES.len();
                            }
                            _ => {
                                let len = match form.active_field {
                                    0 => form.name.chars().count(),
                                    2 => form.provider.chars().count(),
                                    3 => form.base_url.chars().count(),
                                    _ => 0,
                                };
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                        },
                        KeyCode::Backspace => {
                            if form.cursor > 0 && matches!(form.active_field, 0 | 2 | 3) {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.name,
                                    2 => &mut form.provider,
                                    3 => &mut form.base_url,
                                    _ => &mut form.name,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if matches!(form.active_field, 0 | 2 | 3) {
                                let text = match form.active_field {
                                    0 => &mut form.name,
                                    2 => &mut form.provider,
                                    3 => &mut form.base_url,
                                    _ => &mut form.name,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.report_form.is_some() {
                    let form = app.report_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.report_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 3;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                2 => form.prompt.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if form.active_field == 1 {
                                form.kind_index = (form.kind_index + 1) % app::REPORT_KINDS.len();
                            } else if !form.title.is_empty() {
                                let title = form.title.clone();
                                let kind = app::REPORT_KINDS[form.kind_index].to_string();
                                let prompt = form.prompt.clone();
                                let pid = app.current_project;
                                app.report_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let body = serde_json::json!({
                                            "title": title,
                                            "kind": kind,
                                            "prompt": prompt,
                                        });
                                        match api.create_report(pid, body).await {
                                            Ok(_) => {
                                                if let Ok(rpts) = api.list_reports(pid).await {
                                                    let _ = tx.send(ApiMsg::Reports(rpts)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Report: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => {
                            if form.active_field == 1 {
                                if form.kind_index > 0 {
                                    form.kind_index -= 1;
                                } else {
                                    form.kind_index = app::REPORT_KINDS.len() - 1;
                                }
                            } else if form.cursor > 0 {
                                form.cursor -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if form.active_field == 1 {
                                form.kind_index = (form.kind_index + 1) % app::REPORT_KINDS.len();
                            } else {
                                let text = if form.active_field == 0 {
                                    &form.title
                                } else {
                                    &form.prompt
                                };
                                if form.cursor < text.chars().count() {
                                    form.cursor += 1;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if form.cursor > 0 && form.active_field != 1 {
                                form.cursor -= 1;
                                let text = if form.active_field == 0 {
                                    &mut form.title
                                } else {
                                    &mut form.prompt
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if form.active_field != 1 {
                                let text = if form.active_field == 0 {
                                    &mut form.title
                                } else {
                                    &mut form.prompt
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.webhook_form.is_some() {
                    let form = app.webhook_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.webhook_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 3;
                            form.cursor = match form.active_field {
                                0 => form.url.chars().count(),
                                1 => form.secret.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.url.is_empty() {
                                let url = form.url.clone();
                                let secret = if form.secret.is_empty() {
                                    None
                                } else {
                                    Some(form.secret.clone())
                                };
                                let events: Vec<String> = app::WEBHOOK_EVENT_TYPES
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, _)| {
                                        form.event_toggles.get(*i).copied().unwrap_or(false)
                                    })
                                    .map(|(_, ev)| ev.to_string())
                                    .collect();
                                let pid = app.current_project;
                                app.webhook_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let mut body = serde_json::json!({
                                            "url": url,
                                            "events": events,
                                            "enabled": true,
                                        });
                                        if let Some(s) = secret {
                                            body["secret"] = serde_json::json!(s);
                                        }
                                        match api.create_webhook(pid, body).await {
                                            Ok(_) => {
                                                if let Ok(whs) = api.list_webhooks(pid).await {
                                                    let _ = tx.send(ApiMsg::Webhooks(whs)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Webhook: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Up => {
                            if form.active_field == 2 && form.event_selected > 0 {
                                form.event_selected -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if form.active_field == 2
                                && form.event_selected + 1 < app::WEBHOOK_EVENT_TYPES.len()
                            {
                                form.event_selected += 1;
                            }
                        }
                        KeyCode::Char(' ') => {
                            if form.active_field == 2 {
                                let idx = form.event_selected;
                                if idx < form.event_toggles.len() {
                                    form.event_toggles[idx] = !form.event_toggles[idx];
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if form.cursor > 0 && matches!(form.active_field, 0 | 1) {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.url,
                                    1 => &mut form.secret,
                                    _ => &mut form.url,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if matches!(form.active_field, 0 | 1) {
                                let text = match form.active_field {
                                    0 => &mut form.url,
                                    1 => &mut form.secret,
                                    _ => &mut form.url,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.event_form.is_some() {
                    let form = app.event_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.event_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 4;
                            form.cursor = match form.active_field {
                                0 => form.title.chars().count(),
                                3 => form.description.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let title = form.title.clone();
                                let description = form.description.clone();
                                let kind = EVENT_KINDS[form.kind_index].to_string();
                                let severity = EVENT_SEVERITIES[form.severity_index].to_string();
                                let pid = app.current_project;
                                app.event_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let body = serde_json::json!({
                                            "kind": kind,
                                            "severity": severity,
                                            "title": title,
                                            "description": description,
                                            "source": "tui",
                                        });
                                        match api.create_event(pid, body).await {
                                            Ok(_) => {
                                                if let Ok(evts) =
                                                    api.list_events(pid, None, None).await
                                                {
                                                    let _ = tx.send(ApiMsg::Events(evts)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Event: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => match form.active_field {
                            1 => {
                                if form.kind_index > 0 {
                                    form.kind_index -= 1;
                                } else {
                                    form.kind_index = EVENT_KINDS.len() - 1;
                                }
                            }
                            2 => {
                                if form.severity_index > 0 {
                                    form.severity_index -= 1;
                                } else {
                                    form.severity_index = EVENT_SEVERITIES.len() - 1;
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Right => match form.active_field {
                            1 => {
                                form.kind_index = (form.kind_index + 1) % EVENT_KINDS.len();
                            }
                            2 => {
                                form.severity_index =
                                    (form.severity_index + 1) % EVENT_SEVERITIES.len();
                            }
                            _ => {}
                        },
                        KeyCode::Backspace => {
                            if form.cursor > 0 && matches!(form.active_field, 0 | 3) {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    3 => &mut form.description,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if matches!(form.active_field, 0 | 3) {
                                let text = match form.active_field {
                                    0 => &mut form.title,
                                    3 => &mut form.description,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else if app.view == View::ProjectSettings && app.settings_form.is_some() {
                    // Settings form handles its own keys
                    let playbook_count = app.playbooks.len();
                    let form = app.settings_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.settings_form = None;
                            app.view = View::Tasks;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % app::SETTINGS_FIELD_COUNT;
                            // Reset cursor to end of target text field
                            form.cursor = match form.active_field {
                                0 => form.name.chars().count(),
                                1 => form.description.chars().count(),
                                2 => form.repo_url.chars().count(),
                                3 => form.repo_path.chars().count(),
                                4 => form.default_branch.chars().count(),
                                5 => form.service_name.chars().count(),
                                7 => form.claude_md.chars().count(),
                                _ => 0, // playbook dropdown
                            };
                        }
                        KeyCode::BackTab => {
                            form.active_field = if form.active_field == 0 {
                                app::SETTINGS_FIELD_COUNT - 1
                            } else {
                                form.active_field - 1
                            };
                            form.cursor = match form.active_field {
                                0 => form.name.chars().count(),
                                1 => form.description.chars().count(),
                                2 => form.repo_url.chars().count(),
                                3 => form.repo_path.chars().count(),
                                4 => form.default_branch.chars().count(),
                                5 => form.service_name.chars().count(),
                                7 => form.claude_md.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Left => {
                            if form.active_field == 6 {
                                // Playbook dropdown
                                if form.playbook_index > 0 {
                                    form.playbook_index -= 1;
                                } else {
                                    form.playbook_index = playbook_count;
                                }
                                form.dirty = true;
                            } else if form.cursor > 0 {
                                form.cursor -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if form.active_field == 6 {
                                form.playbook_index =
                                    (form.playbook_index + 1) % (playbook_count + 1);
                                form.dirty = true;
                            } else {
                                let len = form.field_text().chars().count();
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                        }
                        KeyCode::Enter => {
                            if form.active_field == 7 {
                                // In CLAUDE.md field, Enter inserts newline
                                let cur = form.cursor;
                                if let Some(text) = form.field_text_mut() {
                                    let bp = text
                                        .char_indices()
                                        .nth(cur)
                                        .map(|(i, _)| i)
                                        .unwrap_or(text.len());
                                    text.insert(bp, '\n');
                                }
                                form.cursor += 1;
                                form.dirty = true;
                            }
                            // For other fields, Enter does nothing (use Ctrl+S to save)
                        }
                        KeyCode::Backspace => {
                            if form.active_field != 6 && form.cursor > 0 {
                                form.cursor -= 1;
                                let cur = form.cursor;
                                if let Some(text) = form.field_text_mut() {
                                    let bp = text
                                        .char_indices()
                                        .nth(cur)
                                        .map(|(i, _)| i)
                                        .unwrap_or(text.len());
                                    text.remove(bp);
                                }
                                form.dirty = true;
                            }
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Save settings
                            if let Some(pid) = app.current_project {
                                let name = form.name.clone();
                                let description = form.description.clone();
                                let repo_url = form.repo_url.clone();
                                let repo_path = form.repo_path.clone();
                                let default_branch = form.default_branch.clone();
                                let service_name = form.service_name.clone();
                                let playbook_id = if form.playbook_index > 0 {
                                    app.playbooks.get(form.playbook_index - 1).map(|pb| pb.id)
                                } else {
                                    None
                                };
                                let claude_md = form.claude_md.clone();
                                form.dirty = false;

                                let api = api.clone();
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    // Update project properties
                                    let mut body = serde_json::json!({
                                        "name": name,
                                        "description": description,
                                        "repo_url": if repo_url.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(repo_url) },
                                        "repo_path": if repo_path.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(repo_path) },
                                        "default_branch": if default_branch.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(default_branch) },
                                        "service_name": if service_name.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(service_name) },
                                    });
                                    body["default_playbook_id"] = match playbook_id {
                                        Some(id) => serde_json::json!(id),
                                        None => serde_json::Value::Null,
                                    };

                                    match api.update_project(pid, body).await {
                                        Ok(proj) => {
                                            let _ = tx.send(ApiMsg::ProjectUpdated(proj)).await;
                                        }
                                        Err(e) => {
                                            let _ = tx
                                                .send(ApiMsg::Error(format!("Save project: {e}")))
                                                .await;
                                        }
                                    }

                                    // Update CLAUDE.md
                                    if let Err(e) = api.update_claude_md(pid, &claude_md).await {
                                        let _ = tx
                                            .send(ApiMsg::Error(format!("Save CLAUDE.md: {e}")))
                                            .await;
                                    }
                                });
                            }
                        }
                        KeyCode::Char(c) => {
                            if form.active_field != 6 {
                                // Text fields
                                let cur = form.cursor;
                                if let Some(text) = form.field_text_mut() {
                                    let bp = text
                                        .char_indices()
                                        .nth(cur)
                                        .map(|(i, _)| i)
                                        .unwrap_or(text.len());
                                    text.insert(bp, c);
                                }
                                form.cursor += 1;
                                form.dirty = true;
                            }
                        }
                        _ => {}
                    }
                } else if app.verification_form.is_some() {
                    let task_count = app.tasks.len().max(1);
                    let form = app.verification_form.as_mut().unwrap();
                    match key.code {
                        KeyCode::Esc => {
                            app.verification_form = None;
                        }
                        KeyCode::Tab => {
                            form.active_field = (form.active_field + 1) % 6;
                            form.cursor = match form.active_field {
                                3 => form.title.chars().count(),
                                4 => form.detail.chars().count(),
                                5 => form.evidence.chars().count(),
                                _ => 0,
                            };
                        }
                        KeyCode::Enter => {
                            if !form.title.is_empty() {
                                let task_id = app.tasks.get(form.task_index).map(|t| t.id);
                                let kind = VERIFICATION_KINDS[form.kind_index].to_string();
                                let status = VERIFICATION_STATUSES[form.status_index].to_string();
                                let title = form.title.clone();
                                let detail = if form.detail.is_empty() {
                                    None
                                } else {
                                    Some(form.detail.clone())
                                };
                                let evidence: Option<serde_json::Value> =
                                    if form.evidence.is_empty() {
                                        None
                                    } else {
                                        serde_json::from_str(&form.evidence).ok()
                                    };
                                let editing_id = form.editing_id;
                                let pid = app.current_project;
                                app.verification_form = None;
                                if let Some(pid) = pid {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        let mut body = serde_json::json!({
                                            "kind": kind,
                                            "status": status,
                                            "title": title,
                                        });
                                        if let Some(tid) = task_id {
                                            body["task_id"] = serde_json::json!(tid);
                                        }
                                        if let Some(d) = detail {
                                            body["detail"] = serde_json::json!(d);
                                        }
                                        if let Some(e) = evidence {
                                            body["evidence"] = e;
                                        }
                                        let result = if let Some(vid) = editing_id {
                                            api.update_verification(vid, body).await.map(|_| ())
                                        } else {
                                            api.create_verification(pid, body).await.map(|_| ())
                                        };
                                        match result {
                                            Ok(()) => {
                                                if let Ok(vs) = api
                                                    .list_verifications(
                                                        pid, None, None, None, 100, 0,
                                                    )
                                                    .await
                                                {
                                                    let _ =
                                                        tx.send(ApiMsg::Verifications(vs)).await;
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!(
                                                        "Verification: {e}"
                                                    )))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Left => {
                            match form.active_field {
                                0 => {
                                    // Cycle task
                                    if form.task_index > 0 {
                                        form.task_index -= 1;
                                    } else {
                                        form.task_index = task_count - 1;
                                    }
                                }
                                1 => {
                                    // Cycle kind
                                    if form.kind_index > 0 {
                                        form.kind_index -= 1;
                                    } else {
                                        form.kind_index = VERIFICATION_KINDS.len() - 1;
                                    }
                                }
                                2 => {
                                    // Cycle status
                                    if form.status_index > 0 {
                                        form.status_index -= 1;
                                    } else {
                                        form.status_index = VERIFICATION_STATUSES.len() - 1;
                                    }
                                }
                                _ => {
                                    // Text field cursor left
                                    if form.cursor > 0 {
                                        form.cursor -= 1;
                                    }
                                }
                            }
                        }
                        KeyCode::Right => match form.active_field {
                            0 => {
                                form.task_index = (form.task_index + 1) % task_count;
                            }
                            1 => {
                                form.kind_index = (form.kind_index + 1) % VERIFICATION_KINDS.len();
                            }
                            2 => {
                                form.status_index =
                                    (form.status_index + 1) % VERIFICATION_STATUSES.len();
                            }
                            _ => {
                                let len = match form.active_field {
                                    3 => form.title.chars().count(),
                                    4 => form.detail.chars().count(),
                                    5 => form.evidence.chars().count(),
                                    _ => 0,
                                };
                                if form.cursor < len {
                                    form.cursor += 1;
                                }
                            }
                        },
                        KeyCode::Backspace => {
                            if form.active_field >= 3 && form.cursor > 0 {
                                form.cursor -= 1;
                                let text = match form.active_field {
                                    3 => &mut form.title,
                                    4 => &mut form.detail,
                                    5 => &mut form.evidence,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.remove(bp);
                            }
                        }
                        KeyCode::Char(c) => {
                            if form.active_field >= 3 {
                                let text = match form.active_field {
                                    3 => &mut form.title,
                                    4 => &mut form.detail,
                                    5 => &mut form.evidence,
                                    _ => &mut form.title,
                                };
                                let bp = text
                                    .char_indices()
                                    .nth(form.cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(text.len());
                                text.insert(bp, c);
                                form.cursor += 1;
                            }
                        }
                        _ => {}
                    }
                } else {
                    match app.modal {
                        Modal::Transition => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(state) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if let Some(tid) = app.selected_task_id() {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            let _ = api.transition_task(tid, &state).await;
                                            if let Some(pid) = pid {
                                                if let Ok(resp) = api.list_tasks(pid).await {
                                                    let _ = tx.send(ApiMsg::Tasks(resp.data)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::Reply => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                if !app.input_text.is_empty() {
                                    if let Some(idx) = app.selected_task {
                                        let task = &app.tasks[idx];
                                        let tid = task.id;
                                        let is_backlog = task.state == "backlog";
                                        let content = app.input_text.clone();
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        if is_backlog {
                                            // Append to spec so additions become part of the task description
                                            let existing_spec = task
                                                .context
                                                .get("spec")
                                                .and_then(|s| s.as_str())
                                                .unwrap_or("");
                                            let new_spec = if existing_spec.is_empty() {
                                                content
                                            } else {
                                                format!("{}\n{}", existing_spec, content)
                                            };
                                            let mut new_context = task.context.clone();
                                            new_context["spec"] =
                                                serde_json::Value::String(new_spec);
                                            let pid = app.current_project;
                                            tokio::spawn(async move {
                                                let _ = api
                                                    .update_task(
                                                        tid,
                                                        serde_json::json!({ "context": new_context }),
                                                    )
                                                    .await;
                                                // Refresh task list
                                                if let Some(pid) = pid {
                                                    if let Ok(resp) = api.list_tasks(pid).await {
                                                        let _ =
                                                            tx.send(ApiMsg::Tasks(resp.data)).await;
                                                    }
                                                }
                                            });
                                        } else {
                                            tokio::spawn(async move {
                                                let _ = api
                                                    .post_update(tid, &content, "progress")
                                                    .await;
                                                if let Ok(updates) = api.get_task_updates(tid).await
                                                {
                                                    let _ =
                                                        tx.send(ApiMsg::TaskUpdates(updates)).await;
                                                }
                                            });
                                        }
                                    }
                                }
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Left => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if app.input_cursor < app.input_text.chars().count() {
                                    app.input_cursor += 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::Comment => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                if !app.input_text.is_empty() {
                                    if let Some(tid) = app.selected_task_id() {
                                        let content = app.input_text.clone();
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            let _ = api.post_comment(tid, &content).await;
                                            if let Ok(comments) = api.get_task_comments(tid).await {
                                                let _ =
                                                    tx.send(ApiMsg::TaskComments(comments)).await;
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Left => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if app.input_cursor < app.input_text.chars().count() {
                                    app.input_cursor += 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::GoalComment => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                if !app.input_text.is_empty() {
                                    if let Some(goal) =
                                        app.selected_goal.and_then(|i| app.goals.get(i))
                                    {
                                        let gid = goal.id;
                                        let content = app.input_text.clone();
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            let _ = api
                                                .create_goal_comment(
                                                    gid,
                                                    serde_json::json!({"content": content}),
                                                )
                                                .await;
                                            if let Ok(comments) = api.list_goal_comments(gid).await
                                            {
                                                let _ =
                                                    tx.send(ApiMsg::GoalComments(comments)).await;
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Left => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if app.input_cursor < app.input_text.chars().count() {
                                    app.input_cursor += 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::LogQuery => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                if !app.input_text.is_empty() {
                                    app.log_query = app.input_text.clone();
                                    app.log_loading = true;
                                    app.log_scroll = 0;
                                    // Fire the query
                                    let query = app.log_query.clone();
                                    let range_secs = TIME_RANGES[app.log_time_range_idx].1;
                                    let limit = LOG_LIMITS[app.log_limit_idx];
                                    let direction =
                                        LOG_DIRECTIONS[app.log_direction_idx].to_string();
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    let end_time = chrono::Utc::now();
                                    let start_time =
                                        end_time - chrono::Duration::seconds(range_secs);
                                    let start_str = start_time.to_rfc3339();
                                    let end_str = end_time.to_rfc3339();
                                    tokio::spawn(async move {
                                        match api
                                            .query_logs(
                                                &query,
                                                Some(&start_str),
                                                Some(&end_str),
                                                limit,
                                                &direction,
                                            )
                                            .await
                                        {
                                            Ok(resp) => {
                                                let _ = tx.send(ApiMsg::Logs(resp)).await;
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Logs: {e}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Left => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if app.input_cursor < app.input_text.chars().count() {
                                    app.input_cursor += 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::LogFilter => match key.code {
                            KeyCode::Esc => {
                                app.log_filter.clear();
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                app.log_filter = app.input_text.clone();
                                app.log_scroll = 0;
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Left => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if app.input_cursor < app.input_text.chars().count() {
                                    app.input_cursor += 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::Search => match key.code {
                            KeyCode::Esc => {
                                app.search_query.clear();
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                                // Reset selection to first visible
                                if !app.tasks.is_empty() {
                                    app.selected_task = Some(0);
                                    app.task_list_state.select(Some(0));
                                }
                            }
                            KeyCode::Enter => {
                                app.search_query = app.input_text.clone();
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                                // Reset selection to first matching task
                                let visible = app.filtered_task_indices();
                                if let Some(&first) = visible.first() {
                                    app.selected_task = Some(first);
                                    app.task_list_state.select(Some(0));
                                }
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::GlobalSearch => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                let query = app.input_text.clone();
                                app.modal = Modal::None;
                                app.view = View::Search;
                                app.input_text.clear();
                                app.input_cursor = 0;
                                if !query.is_empty() {
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.search(pid, &query, None).await {
                                                Ok(resp) => {
                                                    let _ =
                                                        tx.send(ApiMsg::SearchResults(resp)).await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!("Search: {e}")))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::ChatInput => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                            }
                            KeyCode::Enter => {
                                let msg = app.chat_input.trim().to_string();
                                app.modal = Modal::None;
                                if !msg.is_empty() {
                                    app.chat_messages.push(client::ChatMessage {
                                        role: "user".into(),
                                        content: msg,
                                    });
                                    app.chat_input.clear();
                                    app.chat_streaming = true;
                                    let messages = app.chat_messages.clone();
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.send_chat(pid, messages).await {
                                                Ok(content) => {
                                                    let _ = tx
                                                        .send(ApiMsg::ChatResponse(content))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::ChatError(format!("{e}")))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                app.chat_input.pop();
                            }
                            KeyCode::Char(c) => {
                                app.chat_input.push(c);
                            }
                            _ => {}
                        },
                        Modal::ObservationStatus => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(status) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if let Some(obs) = app
                                        .selected_observation
                                        .and_then(|i| app.observations.get(i))
                                    {
                                        let obs_id = obs.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api
                                                .update_observation(
                                                    obs_id,
                                                    serde_json::json!({"status": status}),
                                                )
                                                .await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Status: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(obs) = api.list_observations(pid).await {
                                                    let _ =
                                                        tx.send(ApiMsg::Observations(obs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::DecisionSupersede => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected_title) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    // Find the superseding decision by title
                                    if let Some(superseding) =
                                        app.decisions.iter().find(|d| d.title == selected_title)
                                    {
                                        let superseding_id = superseding.id;
                                        // Mark current decision as superseded
                                        if let Some(dec) =
                                            app.selected_decision.and_then(|i| app.decisions.get(i))
                                        {
                                            let dec_id = dec.id;
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            let pid = app.current_project;
                                            tokio::spawn(async move {
                                                if let Err(e) = api
                                                    .update_decision(
                                                        dec_id,
                                                        serde_json::json!({
                                                            "status": "superseded",
                                                            "superseded_by": superseding_id
                                                        }),
                                                    )
                                                    .await
                                                {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Supersede: {e}"
                                                        )))
                                                        .await;
                                                } else if let Some(pid) = pid {
                                                    if let Ok(decs) = api.list_decisions(pid).await
                                                    {
                                                        let _ =
                                                            tx.send(ApiMsg::Decisions(decs)).await;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::IntegrationAccess => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(agent_name) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if let Some(intg) = app
                                        .selected_integration
                                        .and_then(|i| app.integrations.get(i))
                                    {
                                        let intg_id = intg.id;
                                        // Find agent by name
                                        if let Some(agent) =
                                            app.agents.iter().find(|a| a.name == agent_name)
                                        {
                                            let agent_id = agent.id;
                                            // Check if agent already has access
                                            let has_access = app
                                                .integration_access
                                                .iter()
                                                .any(|a| a.agent_id == agent_id);
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                let result = if has_access {
                                                    api.revoke_integration_access(intg_id, agent_id)
                                                        .await
                                                } else {
                                                    api.grant_integration_access(intg_id, agent_id)
                                                        .await
                                                };
                                                if let Err(e) = result {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!("Access: {e}")))
                                                        .await;
                                                }
                                                // Refresh access list
                                                if let Ok(access) =
                                                    api.list_integration_access(intg_id).await
                                                {
                                                    let _ = tx
                                                        .send(ApiMsg::IntegrationAccessList(access))
                                                        .await;
                                                }
                                            });
                                        }
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::ViewPicker => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if app.transition_selected < ALL_VIEWS.len() {
                                    app.view = ALL_VIEWS[app.transition_selected];
                                    app.detail_scroll = 0;
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::VerificationStatus => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(new_status) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    let filtered = app.filtered_verifications();
                                    if let Some(v) =
                                        app.selected_verification.and_then(|i| filtered.get(i))
                                    {
                                        let vid = v.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            let body = serde_json::json!({"status": new_status});
                                            if let Err(e) = api.update_verification(vid, body).await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Status: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(vs) = api
                                                    .list_verifications(
                                                        pid, None, None, None, 100, 0,
                                                    )
                                                    .await
                                                {
                                                    let _ =
                                                        tx.send(ApiMsg::Verifications(vs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::VerificationKindFilter => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if selected == "all" {
                                        app.verification_kind_filter = None;
                                    } else {
                                        app.verification_kind_filter = Some(selected);
                                    }
                                    app.selected_verification = None;
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::VerificationStatusFilter => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if selected == "all" {
                                        app.verification_status_filter = None;
                                    } else {
                                        app.verification_status_filter = Some(selected);
                                    }
                                    app.selected_verification = None;
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::EventKindFilter => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if selected == "all" {
                                        app.event_kind_filter = None;
                                    } else {
                                        app.event_kind_filter = Some(selected);
                                    }
                                    app.selected_event = None;
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::EventSeverityFilter => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if selected == "all" {
                                        app.event_severity_filter = None;
                                    } else {
                                        app.event_severity_filter = Some(selected);
                                    }
                                    app.selected_event = None;
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::ReportDelete => match key.code {
                            KeyCode::Esc | KeyCode::Char('n') => {
                                app.modal = Modal::None;
                            }
                            KeyCode::Enter | KeyCode::Char('y') => {
                                if let Some(rpt) =
                                    app.selected_report.and_then(|i| app.reports.get(i))
                                {
                                    let rpt_id = rpt.id;
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    let pid = app.current_project;
                                    tokio::spawn(async move {
                                        if let Err(e) = api.delete_report(rpt_id).await {
                                            let _ = tx
                                                .send(ApiMsg::Error(format!("Delete: {e}")))
                                                .await;
                                        } else if let Some(pid) = pid {
                                            if let Ok(rpts) = api.list_reports(pid).await {
                                                let _ = tx.send(ApiMsg::Reports(rpts)).await;
                                            }
                                        }
                                    });
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::GoalLink | Modal::GoalStatus | Modal::DependencyAdd => {
                            match key.code {
                                KeyCode::Esc => app.modal = Modal::None,
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.transition_selected > 0 {
                                        app.transition_selected -= 1;
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if app.transition_selected + 1 < app.transition_options.len() {
                                        app.transition_selected += 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(selected) =
                                        app.transition_options.get(app.transition_selected).cloned()
                                    {
                                        if app.modal == Modal::GoalLink {
                                            // Link selected task to the chosen goal
                                            if let Some(tid) = app.selected_task_id() {
                                                // Find goal by title match
                                                if let Some(goal) =
                                                    app.goals.iter().find(|g| g.title == selected)
                                                {
                                                    let gid = goal.id;
                                                    let api = api.clone();
                                                    let tx = tx.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) =
                                                            api.link_task_to_goal(gid, tid).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::Error(format!(
                                                                    "Link: {e}"
                                                                )))
                                                                .await;
                                                        }
                                                    });
                                                }
                                            }
                                        } else if app.modal == Modal::DependencyAdd {
                                            // Add dependency: find task by title match
                                            if let Some(tid) = app.selected_task_id() {
                                                if let Some(dep_task) =
                                                    app.tasks.iter().find(|t| t.title == selected)
                                                {
                                                    let dep_id = dep_task.id;
                                                    let api = api.clone();
                                                    let tx = tx.clone();
                                                    tokio::spawn(async move {
                                                        if let Err(e) =
                                                            api.add_dependency(tid, dep_id).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::Error(format!(
                                                                    "Add dep: {e}"
                                                                )))
                                                                .await;
                                                        } else if let Ok(deps) =
                                                            api.list_task_dependencies(tid).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::TaskDependencies(
                                                                    deps,
                                                                ))
                                                                .await;
                                                        }
                                                    });
                                                }
                                            }
                                        } else {
                                            // GoalStatus: update goal status
                                            if let Some(gid) = app
                                                .selected_goal
                                                .and_then(|i| app.goals.get(i))
                                                .map(|g| g.id)
                                            {
                                                let api = api.clone();
                                                let tx = tx.clone();
                                                let pid = app.current_project;
                                                tokio::spawn(async move {
                                                    let _ = api
                                                        .update_goal(
                                                            gid,
                                                            serde_json::json!({"status": selected}),
                                                        )
                                                        .await;
                                                    if let Some(pid) = pid {
                                                        if let Ok(goals) = api.list_goals(pid).await
                                                        {
                                                            let _ =
                                                                tx.send(ApiMsg::Goals(goals)).await;
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                    app.modal = Modal::None;
                                }
                                _ => {}
                            }
                        }
                        Modal::GoalTaskPicker => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.goal_unlinked_tasks.clear();
                                app.goal_picker_checked.clear();
                                app.goal_picker_selected = 0;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.goal_picker_selected > 0 {
                                    app.goal_picker_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.goal_picker_selected + 1 < app.goal_unlinked_tasks.len() {
                                    app.goal_picker_selected += 1;
                                }
                            }
                            KeyCode::Char(' ') => {
                                let idx = app.goal_picker_selected;
                                if idx < app.goal_unlinked_tasks.len() {
                                    if app.goal_picker_checked.contains(&idx) {
                                        app.goal_picker_checked.remove(&idx);
                                    } else {
                                        app.goal_picker_checked.insert(idx);
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                if !app.goal_picker_checked.is_empty() {
                                    let task_ids: Vec<uuid::Uuid> = app
                                        .goal_picker_checked
                                        .iter()
                                        .filter_map(|&i| {
                                            app.goal_unlinked_tasks.get(i).map(|t| t.id)
                                        })
                                        .collect();
                                    if let Some(goal) =
                                        app.selected_goal.and_then(|i| app.goals.get(i))
                                    {
                                        let gid = goal.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.bulk_link_tasks(gid, &task_ids).await {
                                                Ok(_) => {
                                                    let _ = tx.send(ApiMsg::GoalBulkLinked).await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Bulk link: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                                app.goal_unlinked_tasks.clear();
                                app.goal_picker_checked.clear();
                                app.goal_picker_selected = 0;
                            }
                            _ => {}
                        },
                        Modal::Promote => match key.code {
                            KeyCode::Esc => {
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Enter => {
                                if !app.input_text.is_empty() {
                                    if let Some(obs) = app
                                        .selected_observation
                                        .and_then(|i| app.observations.get(i))
                                    {
                                        let obs_id = obs.id;
                                        let title = app.input_text.clone();
                                        let kind =
                                            obs.kind.clone().unwrap_or_else(|| "chore".into());
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api
                                                .promote_observation(obs_id, &title, &kind, 3)
                                                .await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Promote: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                let (obs, tasks) = tokio::join!(
                                                    api.list_observations(pid),
                                                    api.list_tasks(pid),
                                                );
                                                if let Ok(obs) = obs {
                                                    let _ =
                                                        tx.send(ApiMsg::Observations(obs)).await;
                                                }
                                                if let Ok(tasks) = tasks {
                                                    let _ =
                                                        tx.send(ApiMsg::Tasks(tasks.data)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                                app.input_text.clear();
                                app.input_cursor = 0;
                            }
                            KeyCode::Backspace => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                    let byte_pos = app
                                        .input_text
                                        .char_indices()
                                        .nth(app.input_cursor)
                                        .map(|(i, _)| i)
                                        .unwrap_or(app.input_text.len());
                                    app.input_text.remove(byte_pos);
                                }
                            }
                            KeyCode::Left => {
                                if app.input_cursor > 0 {
                                    app.input_cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if app.input_cursor < app.input_text.chars().count() {
                                    app.input_cursor += 1;
                                }
                            }
                            KeyCode::Char(c) => {
                                let byte_pos = app
                                    .input_text
                                    .char_indices()
                                    .nth(app.input_cursor)
                                    .map(|(i, _)| i)
                                    .unwrap_or(app.input_text.len());
                                app.input_text.insert(byte_pos, c);
                                app.input_cursor += 1;
                            }
                            _ => {}
                        },
                        Modal::ClaimTask => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(agent_name) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if let Some(tid) = app.selected_task_id() {
                                        let agent_id = app
                                            .agents
                                            .iter()
                                            .find(|a| a.name == agent_name)
                                            .map(|a| a.id);
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            match api.claim_task(tid, agent_id).await {
                                                Ok(_) => {
                                                    if let Some(pid) = pid {
                                                        if let Ok(resp) = api.list_tasks(pid).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::Tasks(resp.data))
                                                                .await;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!("Claim: {e}")))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::DelegateTask => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(selected) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    if let Some(tid) = app.selected_task_id() {
                                        // Check if it's an agent or role
                                        let agent_id = app
                                            .agents
                                            .iter()
                                            .find(|a| a.name == selected)
                                            .map(|a| a.id);
                                        let role_id = if agent_id.is_none() {
                                            app.roles
                                                .iter()
                                                .find(|r| format!("Role: {}", r.name) == selected)
                                                .map(|r| r.id)
                                        } else {
                                            None
                                        };
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            match api.delegate_task(tid, agent_id, role_id).await {
                                                Ok(_) => {
                                                    if let Some(pid) = pid {
                                                        if let Ok(resp) = api.list_tasks(pid).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::Tasks(resp.data))
                                                                .await;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Delegate: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::BulkTransition => match key.code {
                            KeyCode::Esc => app.modal = Modal::None,
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.transition_selected > 0 {
                                    app.transition_selected -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.transition_selected + 1 < app.transition_options.len() {
                                    app.transition_selected += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(state) =
                                    app.transition_options.get(app.transition_selected).cloned()
                                {
                                    let task_ids: Vec<uuid::Uuid> =
                                        app.bulk_selected.iter().copied().collect();
                                    if !task_ids.is_empty() {
                                        if let Some(pid) = app.current_project {
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                match api
                                                    .bulk_transition(pid, task_ids, &state)
                                                    .await
                                                {
                                                    Ok(()) => {
                                                        if let Ok(resp) = api.list_tasks(pid).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::Tasks(resp.data))
                                                                .await;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let _ = tx
                                                            .send(ApiMsg::Error(format!(
                                                                "Bulk: {e}"
                                                            )))
                                                            .await;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    app.bulk_selected.clear();
                                    app.bulk_mode = false;
                                }
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::BulkDelete => match key.code {
                            KeyCode::Esc | KeyCode::Char('n') => {
                                app.modal = Modal::None;
                            }
                            KeyCode::Enter | KeyCode::Char('y') => {
                                let task_ids: Vec<uuid::Uuid> =
                                    app.bulk_selected.iter().copied().collect();
                                if !task_ids.is_empty() {
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.bulk_delete(pid, task_ids).await {
                                                Ok(()) => {
                                                    if let Ok(resp) = api.list_tasks(pid).await {
                                                        let _ =
                                                            tx.send(ApiMsg::Tasks(resp.data)).await;
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Bulk delete: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                                app.bulk_selected.clear();
                                app.bulk_mode = false;
                                app.modal = Modal::None;
                            }
                            _ => {}
                        },
                        Modal::None => match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('[') | KeyCode::Char(']') => {
                                if !app.projects.is_empty() {
                                    let cur_idx = app
                                        .current_project
                                        .and_then(|pid| {
                                            app.projects.iter().position(|p| p.id == pid)
                                        })
                                        .unwrap_or(0);
                                    let next_idx = if key.code == KeyCode::Char(']') {
                                        (cur_idx + 1) % app.projects.len()
                                    } else if cur_idx == 0 {
                                        app.projects.len() - 1
                                    } else {
                                        cur_idx - 1
                                    };
                                    let pid = app.projects[next_idx].id;
                                    app.current_project = Some(pid);
                                    project_tx.send(Some(pid)).ok();
                                    // Clear stale data
                                    app.tasks.clear();
                                    app.task_updates.clear();
                                    app.task_comments.clear();
                                    app.task_dependencies = client::TaskDependencies {
                                        depends_on: vec![],
                                        blocks: vec![],
                                    };
                                    app.search_query.clear();
                                    app.playbooks.clear();
                                    app.knowledge.clear();
                                    app.decisions.clear();
                                    app.goals.clear();
                                    app.goal_progress = None;
                                    app.goal_stats = None;
                                    app.goal_children.clear();
                                    app.goal_comments.clear();
                                    app.observations.clear();
                                    app.roles.clear();
                                    app.members.clear();
                                    app.integrations.clear();
                                    app.audit_log.clear();
                                    app.log_entries.clear();
                                    app.log_labels.clear();
                                    app.log_scroll = 0;
                                    app.selected_task = None;
                                    app.task_list_state.select(None);
                                    app.selected_goal = None;
                                    app.selected_observation = None;
                                    app.selected_role = None;
                                    app.selected_member = None;
                                    app.selected_integration = None;
                                    app.selected_audit = None;
                                    app.detail_scroll = 0;
                                    app.git_task_status = None;
                                    app.changed_files.clear();
                                    app.show_changed_files = false;
                                    app.bulk_selected.clear();
                                    app.bulk_mode = false;
                                }
                            }
                            KeyCode::Char('1') => {
                                app.view = View::Tasks;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('2') => {
                                app.view = View::Agents;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('3') => {
                                app.view = View::Knowledge;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('4') => {
                                app.view = View::Decisions;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('5') => {
                                app.view = View::Playbooks;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('6') => {
                                app.view = View::Goals;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('7') => {
                                app.view = View::Observations;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('8') => {
                                app.view = View::Team;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('9') => {
                                app.view = View::Integrations;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('0') => {
                                app.view = View::Audit;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('l')
                                if app.view != View::Logs && app.view != View::Tasks =>
                            {
                                // 'l' switches to Logs view (except when already in Logs or Tasks
                                // where it conflicts with nothing, but 'l' is a common char)
                                app.view = View::Logs;
                                app.detail_scroll = 0;
                                // Fetch labels on first visit
                                if app.log_labels.is_empty() {
                                    let api = api.clone();
                                    let tx = tx.clone();
                                    tokio::spawn(async move {
                                        if let Ok(resp) = api.list_log_labels().await {
                                            let _ = tx.send(ApiMsg::LogLabels(resp.data)).await;
                                        }
                                    });
                                }
                            }
                            KeyCode::Char('V') => {
                                app.view = View::Verifications;
                                app.detail_scroll = 0;
                            }
                            KeyCode::Char('C') => {
                                if app.view == View::Observations {
                                    // Cleanup observations
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.cleanup_observations(pid).await {
                                                Ok(result) => {
                                                    let _ = tx
                                                        .send(ApiMsg::ObservationsCleanup(result))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Cleanup: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                } else {
                                    app.view = View::Chat;
                                    app.detail_scroll = 0;
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.view == View::Logs {
                                    app.scroll_logs(-1);
                                } else if app.focus == 1 {
                                    app.scroll_detail(-1);
                                } else if app.view == View::Tasks {
                                    app.move_task_selection(-1);
                                } else {
                                    app.move_selection(-1);
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.view == View::Logs {
                                    app.scroll_logs(1);
                                } else if app.focus == 1 {
                                    app.scroll_detail(1);
                                } else if app.view == View::Tasks {
                                    app.move_task_selection(1);
                                } else {
                                    app.move_selection(1);
                                }
                            }
                            KeyCode::Tab => {
                                if app.view == View::Team {
                                    app.team_focus = (app.team_focus + 1) % 3;
                                } else {
                                    app.focus = (app.focus + 1) % 2;
                                }
                            }
                            KeyCode::Enter => {
                                if app.view == View::Logs {
                                    // Open LogQL query editor
                                    app.modal = Modal::LogQuery;
                                    app.input_text = app.log_query.clone();
                                    app.input_cursor = app.input_text.chars().count();
                                } else if app.view == View::Tasks {
                                    if let Some(tid) = app.selected_task_id() {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            let (updates, comments) = tokio::join!(
                                                api.get_task_updates(tid),
                                                api.get_task_comments(tid),
                                            );
                                            if let Ok(updates) = updates {
                                                let _ = tx.send(ApiMsg::TaskUpdates(updates)).await;
                                            }
                                            if let Ok(comments) = comments {
                                                let _ =
                                                    tx.send(ApiMsg::TaskComments(comments)).await;
                                            }
                                        });
                                    }
                                } else if app.view == View::Source {
                                    if let Some(sel) = app.source_selected {
                                        let offset = if app.source_current_path.is_empty() {
                                            0
                                        } else {
                                            1
                                        };
                                        if sel == 0 && offset == 1 {
                                            // Go up
                                            let new_path = app
                                                .source_current_path
                                                .rfind('/')
                                                .map(|i| app.source_current_path[..i].to_string())
                                                .unwrap_or_default();
                                            app.source_current_path = new_path.clone();
                                            if let Some(pid) = app.current_project {
                                                let api = api.clone();
                                                let tx = tx.clone();
                                                tokio::spawn(async move {
                                                    if let Ok(resp) =
                                                        api.source_tree(pid, &new_path, None).await
                                                    {
                                                        let _ = tx
                                                            .send(ApiMsg::SourceTree(resp.entries))
                                                            .await;
                                                    }
                                                });
                                            }
                                        } else if let Some(entry) =
                                            app.source_entries.get(sel - offset)
                                        {
                                            let entry_path = entry.path.clone();
                                            let entry_kind = entry.kind.clone();
                                            if let Some(pid) = app.current_project {
                                                let api = api.clone();
                                                let tx = tx.clone();
                                                if entry_kind == "dir" {
                                                    app.source_current_path = entry_path.clone();
                                                    tokio::spawn(async move {
                                                        if let Ok(resp) = api
                                                            .source_tree(pid, &entry_path, None)
                                                            .await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::SourceTree(
                                                                    resp.entries,
                                                                ))
                                                                .await;
                                                        }
                                                    });
                                                } else {
                                                    tokio::spawn(async move {
                                                        if let Ok(resp) = api
                                                            .source_blob(pid, &entry_path, None)
                                                            .await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::SourceBlob {
                                                                    path: entry_path,
                                                                    content: resp.content,
                                                                })
                                                                .await;
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('t') => {
                                if app.view == View::Tasks {
                                    if let Some(task) =
                                        app.selected_task.and_then(|i| app.tasks.get(i))
                                    {
                                        let opts = valid_transitions(&task.state);
                                        if !opts.is_empty() {
                                            app.transition_options = opts;
                                            app.transition_selected = 0;
                                            app.modal = Modal::Transition;
                                        }
                                    }
                                } else if app.view == View::Webhooks {
                                    if let Some(wh) =
                                        app.selected_webhook.and_then(|i| app.webhooks.get(i))
                                    {
                                        let wh_id = wh.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.test_webhook(wh_id).await {
                                                Ok(resp) => {
                                                    let summary = resp
                                                        .get("status")
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_string())
                                                        .unwrap_or_else(|| {
                                                            serde_json::to_string(&resp)
                                                                .unwrap_or_else(|_| "OK".into())
                                                        });
                                                    let _ = tx
                                                        .send(ApiMsg::WebhookTestResult(summary))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::WebhookTestResult(format!(
                                                            "Error: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Char('c') => {
                                if app.view == View::Tasks && app.selected_task.is_some() {
                                    app.modal = Modal::Comment;
                                    app.input_text.clear();
                                    app.input_cursor = 0;
                                } else if app.view == View::Goals && app.selected_goal.is_some() {
                                    app.modal = Modal::GoalComment;
                                    app.input_text.clear();
                                    app.input_cursor = 0;
                                }
                            }
                            KeyCode::Char('r') => {
                                if app.view == View::Tasks && app.selected_task.is_some() {
                                    app.modal = Modal::Reply;
                                    app.input_text.clear();
                                    app.input_cursor = 0;
                                } else if app.view == View::Git {
                                    app.git_action_result = None;
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            if let Ok(resp) = api.list_branches(pid, None).await {
                                                let _ = tx.send(ApiMsg::Branches(resp)).await;
                                            }
                                            if let Ok(status) = api.main_status(pid).await {
                                                let _ = tx.send(ApiMsg::MainStatus(status)).await;
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Char('/') => {
                                if app.view == View::Logs {
                                    app.modal = Modal::LogFilter;
                                    app.input_text = app.log_filter.clone();
                                    app.input_cursor = app.input_text.chars().count();
                                } else if app.view == View::Search {
                                    app.modal = Modal::GlobalSearch;
                                    app.input_text.clear();
                                    app.input_cursor = 0;
                                } else {
                                    app.modal = Modal::Search;
                                    app.input_text.clear();
                                    app.input_cursor = 0;
                                }
                            }
                            KeyCode::Char('?') => {
                                app.show_help = true;
                            }
                            KeyCode::Char('n') => {
                                if app.view == View::Tasks {
                                    app.task_form = Some(app::TaskForm {
                                        playbook_index: app.default_playbook_index(),
                                        ..Default::default()
                                    });
                                } else if app.view == View::Playbooks {
                                    app.playbook_form = Some(app::PlaybookForm::default());
                                } else if app.view == View::Verifications {
                                    app.verification_form = Some(VerificationForm::default());
                                } else if app.view == View::Goals {
                                    app.goal_form = Some(app::GoalForm::default());
                                } else if app.view == View::Observations {
                                    app.observation_form = Some(app::ObservationForm::default());
                                } else if app.view == View::Decisions {
                                    app.decision_form = Some(app::DecisionForm::default());
                                } else if app.view == View::Knowledge {
                                    app.knowledge_form = Some(app::KnowledgeForm::default());
                                } else if app.view == View::Integrations {
                                    app.integration_form = Some(app::IntegrationForm::default());
                                } else if app.view == View::Reports {
                                    app.report_form = Some(app::ReportForm::default());
                                } else if app.view == View::Events {
                                    app.event_form = Some(app::EventForm::default());
                                } else if app.view == View::Webhooks {
                                    app.webhook_form = Some(app::WebhookForm::default());
                                }
                            }
                            // Goals view: s = status transition
                            // Observations view: s = status transition
                            KeyCode::Char('s') => {
                                if app.view == View::Goals {
                                    if let Some(goal) =
                                        app.selected_goal.and_then(|i| app.goals.get(i))
                                    {
                                        let status = goal.status.as_deref().unwrap_or("active");
                                        let opts: Vec<String> = match status {
                                            "active" => vec![
                                                "achieved".into(),
                                                "paused".into(),
                                                "abandoned".into(),
                                            ],
                                            "paused" => vec!["active".into(), "abandoned".into()],
                                            "achieved" => vec!["active".into()],
                                            "abandoned" => vec!["active".into()],
                                            _ => vec![
                                                "active".into(),
                                                "achieved".into(),
                                                "paused".into(),
                                                "abandoned".into(),
                                            ],
                                        };
                                        if !opts.is_empty() {
                                            app.transition_options = opts;
                                            app.transition_selected = 0;
                                            app.modal = Modal::GoalStatus;
                                        }
                                    }
                                } else if app.view == View::Observations {
                                    if let Some(obs) = app
                                        .selected_observation
                                        .and_then(|i| app.observations.get(i))
                                    {
                                        let status = obs.status.as_deref().unwrap_or("open");
                                        let opts: Vec<String> = match status {
                                            "open" => vec![
                                                "acknowledged".into(),
                                                "acted_on".into(),
                                                "dismissed".into(),
                                            ],
                                            "acknowledged" => {
                                                vec!["acted_on".into(), "dismissed".into()]
                                            }
                                            "acted_on" => vec!["dismissed".into()],
                                            "dismissed" => {
                                                vec!["open".into(), "acknowledged".into()]
                                            }
                                            _ => vec![
                                                "open".into(),
                                                "acknowledged".into(),
                                                "acted_on".into(),
                                                "dismissed".into(),
                                            ],
                                        };
                                        if !opts.is_empty() {
                                            app.transition_options = opts;
                                            app.transition_selected = 0;
                                            app.modal = Modal::ObservationStatus;
                                        }
                                    }
                                } else if app.view == View::Verifications {
                                    let filtered = app.filtered_verifications();
                                    if let Some(v) =
                                        app.selected_verification.and_then(|i| filtered.get(i))
                                    {
                                        let opts: Vec<String> = VERIFICATION_STATUSES
                                            .iter()
                                            .filter(|s| **s != v.status)
                                            .map(|s| s.to_string())
                                            .collect();
                                        if !opts.is_empty() {
                                            app.transition_options = opts;
                                            app.transition_selected = 0;
                                            app.modal = Modal::VerificationStatus;
                                        }
                                    }
                                }
                            }
                            // Events view: f = kind filter
                            KeyCode::Char('f') if app.view == View::Events => {
                                let mut opts: Vec<String> = vec!["all".into()];
                                opts.extend(EVENT_KINDS.iter().map(|k| k.to_string()));
                                app.transition_options = opts;
                                app.transition_selected = 0;
                                app.modal = Modal::EventKindFilter;
                            }
                            // Verifications view: K = kind filter, S = status filter
                            KeyCode::Char('K') => {
                                if app.view == View::Verifications {
                                    let mut opts: Vec<String> = vec!["all".into()];
                                    opts.extend(VERIFICATION_KINDS.iter().map(|k| k.to_string()));
                                    app.transition_options = opts;
                                    app.transition_selected = 0;
                                    app.modal = Modal::VerificationKindFilter;
                                }
                            }
                            KeyCode::Char('S') => {
                                if app.view == View::Verifications {
                                    let mut opts: Vec<String> = vec!["all".into()];
                                    opts.extend(
                                        VERIFICATION_STATUSES.iter().map(|s| s.to_string()),
                                    );
                                    app.transition_options = opts;
                                    app.transition_selected = 0;
                                    app.modal = Modal::VerificationStatusFilter;
                                } else if app.view == View::Decisions {
                                    if let Some(dec) =
                                        app.selected_decision.and_then(|i| app.decisions.get(i))
                                    {
                                        // Show other decisions to supersede with
                                        let dec_id = dec.id;
                                        let opts: Vec<String> = app
                                            .decisions
                                            .iter()
                                            .filter(|d| d.id != dec_id)
                                            .map(|d| d.title.clone())
                                            .collect();
                                        if !opts.is_empty() {
                                            app.transition_options = opts;
                                            app.transition_selected = 0;
                                            app.modal = Modal::DecisionSupersede;
                                        }
                                    }
                                } else if let Some(pid) = app.current_project {
                                    // Open project settings
                                    let project =
                                        app.projects.iter().find(|p| p.id == pid).cloned();
                                    if let Some(proj) = project {
                                        // Find playbook index
                                        let pb_idx = proj
                                            .default_playbook_id
                                            .and_then(|pb_id| {
                                                app.playbooks.iter().position(|pb| pb.id == pb_id)
                                            })
                                            .map(|i| i + 1)
                                            .unwrap_or(0);

                                        app.settings_form = Some(app::ProjectSettingsForm {
                                            name: proj.name.clone(),
                                            description: proj
                                                .description
                                                .clone()
                                                .unwrap_or_default(),
                                            repo_url: proj.repo_url.clone().unwrap_or_default(),
                                            repo_path: proj.repo_path.clone().unwrap_or_default(),
                                            default_branch: proj
                                                .default_branch
                                                .clone()
                                                .unwrap_or_default(),
                                            service_name: proj
                                                .service_name
                                                .clone()
                                                .unwrap_or_default(),
                                            playbook_index: pb_idx,
                                            claude_md: String::new(),
                                            active_field: 0,
                                            cursor: proj.name.chars().count(),
                                            dirty: false,
                                        });
                                        app.view = View::ProjectSettings;
                                        app.detail_scroll = 0;

                                        // Fetch CLAUDE.md
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.get_claude_md(pid).await {
                                                Ok(content) => {
                                                    let _ =
                                                        tx.send(ApiMsg::ClaudeMd(content)).await;
                                                }
                                                Err(_) => {
                                                    // Not found or error — leave empty
                                                    let _ = tx
                                                        .send(ApiMsg::ClaudeMd(String::new()))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // Tasks view: g = link task to goal
                            KeyCode::Char('g') => {
                                if app.view == View::Tasks
                                    && app.selected_task.is_some()
                                    && !app.goals.is_empty()
                                {
                                    let opts: Vec<String> =
                                        app.goals.iter().map(|g| g.title.clone()).collect();
                                    app.transition_options = opts;
                                    app.transition_selected = 0;
                                    app.modal = Modal::GoalLink;
                                }
                            }
                            // Goals view: l = link tasks (open picker)
                            KeyCode::Char('l') => {
                                if app.view == View::Goals && app.selected_goal.is_some() {
                                    if let Some(pid) = app.current_project {
                                        app.goal_picker_selected = 0;
                                        app.goal_picker_checked.clear();
                                        app.goal_picker_loading = true;
                                        app.goal_unlinked_tasks.clear();
                                        app.modal = Modal::GoalTaskPicker;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.list_unlinked_tasks(pid, 50, 0).await {
                                                Ok(tasks) => {
                                                    let _ = tx
                                                        .send(ApiMsg::GoalUnlinkedTasks(tasks))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Unlinked tasks: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // Tasks view (bulk mode): d = bulk delete, deliveries (Webhooks)
                            // Observations view: d = dismiss
                            KeyCode::Char('d') => {
                                if app.view == View::Webhooks {
                                    if let Some(wh) =
                                        app.selected_webhook.and_then(|i| app.webhooks.get(i))
                                    {
                                        let wh_id = wh.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.list_webhook_deliveries(wh_id).await {
                                                Ok(deliveries) => {
                                                    let _ = tx
                                                        .send(ApiMsg::WebhookDeliveries(deliveries))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Deliveries: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Tasks
                                    && app.bulk_mode
                                    && !app.bulk_selected.is_empty()
                                {
                                    app.modal = Modal::BulkDelete;
                                } else if app.view == View::Observations {
                                    if let Some(obs) = app
                                        .selected_observation
                                        .and_then(|i| app.observations.get(i))
                                    {
                                        let obs_id = obs.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api.dismiss_observation(obs_id).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Dismiss: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(obs) = api.list_observations(pid).await {
                                                    let _ =
                                                        tx.send(ApiMsg::Observations(obs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Char('p') => {
                                if app.view == View::Observations
                                    && app.selected_observation.is_some()
                                {
                                    app.modal = Modal::Promote;
                                    // Pre-fill with observation title
                                    if let Some(obs) = app
                                        .selected_observation
                                        .and_then(|i| app.observations.get(i))
                                    {
                                        app.input_text = obs.title.clone();
                                        app.input_cursor = app.input_text.chars().count();
                                    }
                                } else if app.view == View::Git {
                                    // Push selected branch
                                    if let (Some(pid), Some(idx)) =
                                        (app.current_project, app.selected_branch)
                                    {
                                        if let Some(branch) = app.branches.get(idx) {
                                            let branch_name = branch.name.clone();
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                match api.push_branch(pid, &branch_name).await {
                                                    Ok(resp) => {
                                                        let _ = tx
                                                            .send(ApiMsg::GitActionResult(
                                                                resp.message,
                                                            ))
                                                            .await;
                                                    }
                                                    Err(e) => {
                                                        let _ = tx
                                                            .send(ApiMsg::GitActionResult(format!(
                                                                "Push error: {e}"
                                                            )))
                                                            .await;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            // Tasks view: a = claim/assign task
                            // Integrations view: a = access management
                            // Decisions view: a = accept
                            KeyCode::Char('a') => {
                                if app.view == View::Agents {
                                    if let Some(agent) =
                                        app.selected_agent.and_then(|i| app.agents.get(i))
                                    {
                                        let agent_id = agent.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.list_agent_tasks(agent_id).await {
                                                Ok(tasks) => {
                                                    let _ =
                                                        tx.send(ApiMsg::AgentTasks(tasks)).await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Agent tasks: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Tasks && app.selected_task.is_some() {
                                    // Open claim modal with agent list
                                    let opts: Vec<String> =
                                        app.agents.iter().map(|a| a.name.clone()).collect();
                                    if !opts.is_empty() {
                                        app.transition_options = opts;
                                        app.transition_selected = 0;
                                        app.modal = Modal::ClaimTask;
                                    }
                                } else if app.view == View::Integrations {
                                    // Open access management for selected integration
                                    if let Some(intg) = app
                                        .selected_integration
                                        .and_then(|i| app.integrations.get(i))
                                    {
                                        let intg_id = intg.id;
                                        // Fetch current access list
                                        let api_c = api.clone();
                                        let tx_c = tx.clone();
                                        tokio::spawn(async move {
                                            if let Ok(access) =
                                                api_c.list_integration_access(intg_id).await
                                            {
                                                let _ = tx_c
                                                    .send(ApiMsg::IntegrationAccessList(access))
                                                    .await;
                                            }
                                        });
                                        // Build agent list with access markers
                                        let opts: Vec<String> = app
                                            .agents
                                            .iter()
                                            .map(|a| {
                                                let has_access = app
                                                    .integration_access
                                                    .iter()
                                                    .any(|ac| ac.agent_id == a.id);
                                                if has_access {
                                                    format!("✓ {}", a.name)
                                                } else {
                                                    format!("  {}", a.name)
                                                }
                                            })
                                            .collect();
                                        if !opts.is_empty() {
                                            // Use agent name without marker for matching
                                            let opts_clean: Vec<String> =
                                                app.agents.iter().map(|a| a.name.clone()).collect();
                                            app.transition_options = opts_clean;
                                            app.transition_selected = 0;
                                            app.modal = Modal::IntegrationAccess;
                                        }
                                    }
                                } else if app.view == View::Decisions {
                                    if let Some(dec) =
                                        app.selected_decision.and_then(|i| app.decisions.get(i))
                                    {
                                        let dec_id = dec.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api
                                                .update_decision(
                                                    dec_id,
                                                    serde_json::json!({"status": "accepted"}),
                                                )
                                                .await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Accept: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(decs) = api.list_decisions(pid).await {
                                                    let _ = tx.send(ApiMsg::Decisions(decs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Char('x') => {
                                if app.view == View::Decisions {
                                    if let Some(dec) =
                                        app.selected_decision.and_then(|i| app.decisions.get(i))
                                    {
                                        let dec_id = dec.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api
                                                .update_decision(
                                                    dec_id,
                                                    serde_json::json!({"status": "rejected"}),
                                                )
                                                .await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Reject: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(decs) = api.list_decisions(pid).await {
                                                    let _ = tx.send(ApiMsg::Decisions(decs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // X = deprecate decision (Decisions view)
                            KeyCode::Char('X') => {
                                if app.view == View::Decisions {
                                    if let Some(dec) =
                                        app.selected_decision.and_then(|i| app.decisions.get(i))
                                    {
                                        let dec_id = dec.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api
                                                .update_decision(
                                                    dec_id,
                                                    serde_json::json!({"status": "deprecated"}),
                                                )
                                                .await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Deprecate: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(decs) = api.list_decisions(pid).await {
                                                    let _ = tx.send(ApiMsg::Decisions(decs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // R = release task (Tasks view)
                            KeyCode::Char('R') => {
                                if app.view == View::Tasks {
                                    if let Some(task) =
                                        app.selected_task.and_then(|i| app.tasks.get(i))
                                    {
                                        if task.assigned_agent_id.is_some() {
                                            let tid = task.id;
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            let pid = app.current_project;
                                            tokio::spawn(async move {
                                                match api.release_task(tid).await {
                                                    Ok(_) => {
                                                        if let Some(pid) = pid {
                                                            if let Ok(resp) =
                                                                api.list_tasks(pid).await
                                                            {
                                                                let _ = tx
                                                                    .send(ApiMsg::Tasks(resp.data))
                                                                    .await;
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let _ = tx
                                                            .send(ApiMsg::Error(format!(
                                                                "Release: {e}"
                                                            )))
                                                            .await;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                } else if app.view == View::Git {
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.resolve_and_push_main(pid).await {
                                                Ok(resp) => {
                                                    let _ = tx
                                                        .send(ApiMsg::GitActionResult(resp.message))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::GitActionResult(format!(
                                                            "Resolve+push error: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // v = toggle bulk selection / multi-select (Tasks view)
                            KeyCode::Char('v') => {
                                if app.view == View::Tasks {
                                    if !app.bulk_mode {
                                        app.bulk_mode = true;
                                        app.bulk_selected.clear();
                                        // Select current task
                                        if let Some(tid) = app.selected_task_id() {
                                            app.bulk_selected.insert(tid);
                                        }
                                    } else {
                                        // Toggle current task in selection
                                        if let Some(tid) = app.selected_task_id() {
                                            if app.bulk_selected.contains(&tid) {
                                                app.bulk_selected.remove(&tid);
                                            } else {
                                                app.bulk_selected.insert(tid);
                                            }
                                        }
                                    }
                                }
                            }
                            // Esc in bulk mode = exit bulk mode (handled in normal Esc too)
                            KeyCode::Esc => {
                                if app.view == View::Tasks && app.bulk_mode {
                                    app.bulk_mode = false;
                                    app.bulk_selected.clear();
                                }
                            }
                            // b = bulk transition (Tasks view, when in bulk mode)
                            KeyCode::Char('b') => {
                                if app.view == View::Tasks
                                    && app.bulk_mode
                                    && !app.bulk_selected.is_empty()
                                {
                                    let opts = vec![
                                        "ready".into(),
                                        "backlog".into(),
                                        "done".into(),
                                        "cancelled".into(),
                                    ];
                                    app.transition_options = opts;
                                    app.transition_selected = 0;
                                    app.modal = Modal::BulkTransition;
                                }
                            }
                            // f = toggle flagged (Tasks view)
                            KeyCode::Char('f') => {
                                if app.view == View::Tasks {
                                    if let Some(task) =
                                        app.selected_task.and_then(|i| app.tasks.get(i))
                                    {
                                        let tid = task.id;
                                        let new_flagged = !task.flagged;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            match api
                                                .update_task(
                                                    tid,
                                                    serde_json::json!({"flagged": new_flagged}),
                                                )
                                                .await
                                            {
                                                Ok(_) => {
                                                    if let Some(pid) = pid {
                                                        if let Ok(resp) = api.list_tasks(pid).await
                                                        {
                                                            let _ = tx
                                                                .send(ApiMsg::Tasks(resp.data))
                                                                .await;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!("Flag: {e}")))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // F = view changed files (Tasks view), severity filter (Events view)
                            KeyCode::Char('F') => {
                                if app.view == View::Events {
                                    let mut opts: Vec<String> = vec!["all".into()];
                                    opts.extend(EVENT_SEVERITIES.iter().map(|s| s.to_string()));
                                    app.transition_options = opts;
                                    app.transition_selected = 0;
                                    app.modal = Modal::EventSeverityFilter;
                                } else if app.view == View::Tasks {
                                    if let Some(tid) = app.selected_task_id() {
                                        app.show_changed_files = !app.show_changed_files;
                                        if app.show_changed_files {
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                let (status, files) = tokio::join!(
                                                    api.get_git_task_status(tid),
                                                    api.get_changed_files(tid),
                                                );
                                                if let Ok(status) = status {
                                                    let _ = tx
                                                        .send(ApiMsg::GitTaskStatus(status))
                                                        .await;
                                                }
                                                if let Ok(files) = files {
                                                    let _ =
                                                        tx.send(ApiMsg::ChangedFiles(files)).await;
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            // G = git status for task (Tasks view) or switch to Git view
                            KeyCode::Char('G') => {
                                if app.view == View::Tasks {
                                    if let Some(tid) = app.selected_task_id() {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.get_git_task_status(tid).await {
                                                Ok(status) => {
                                                    let _ = tx
                                                        .send(ApiMsg::GitTaskStatus(status))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Git status: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                } else {
                                    app.view = View::Git;
                                    app.detail_scroll = 0;
                                    app.git_action_result = None;
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            if let Ok(resp) = api.list_branches(pid, None).await {
                                                let _ = tx.send(ApiMsg::Branches(resp)).await;
                                            }
                                            if let Ok(status) = api.main_status(pid).await {
                                                let _ = tx.send(ApiMsg::MainStatus(status)).await;
                                            }
                                        });
                                    }
                                }
                            }
                            // i = delegate task (Tasks view)
                            KeyCode::Char('i') => {
                                if app.view == View::Tasks && app.selected_task.is_some() {
                                    // Build list of agents + roles for delegation
                                    let mut opts: Vec<String> =
                                        app.agents.iter().map(|a| a.name.clone()).collect();
                                    for role in &app.roles {
                                        opts.push(format!("Role: {}", role.name));
                                    }
                                    if !opts.is_empty() {
                                        app.transition_options = opts;
                                        app.transition_selected = 0;
                                        app.modal = Modal::DelegateTask;
                                    }
                                } else if app.view == View::Chat {
                                    app.modal = Modal::ChatInput;
                                }
                            }
                            // w = add dependency (Tasks view)
                            KeyCode::Char('w') => {
                                if app.view == View::Tasks && app.selected_task.is_some() {
                                    // Show list of other tasks to pick as dependency
                                    let opts: Vec<String> = app
                                        .tasks
                                        .iter()
                                        .filter(|t| Some(t.id) != app.selected_task_id())
                                        .map(|t| t.title.clone())
                                        .collect();
                                    if !opts.is_empty() {
                                        app.transition_options = opts;
                                        app.transition_selected = 0;
                                        app.modal = Modal::DependencyAdd;
                                    }
                                }
                            }
                            // W = remove first dependency (Tasks view)
                            KeyCode::Char('W') => {
                                if app.view == View::Tasks {
                                    if let Some(tid) = app.selected_task_id() {
                                        // Find first dependency for this task
                                        if let Some(dep) = app.task_dependencies.depends_on.first()
                                        {
                                            let dep_on = dep.depends_on;
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                if let Err(e) =
                                                    api.remove_dependency(tid, dep_on).await
                                                {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Remove dep: {e}"
                                                        )))
                                                        .await;
                                                } else if let Ok(deps) =
                                                    api.list_task_dependencies(tid).await
                                                {
                                                    let _ = tx
                                                        .send(ApiMsg::TaskDependencies(deps))
                                                        .await;
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            // e = edit task (Tasks view), edit playbook (Playbooks view), toggle integration (Integrations view)
                            KeyCode::Char('e') => {
                                if app.view == View::Tasks {
                                    if let Some(task) =
                                        app.selected_task.and_then(|i| app.tasks.get(i))
                                    {
                                        let kind_index = TASK_KINDS
                                            .iter()
                                            .position(|&k| k == task.kind)
                                            .unwrap_or(0);
                                        let spec = task
                                            .context
                                            .get("spec")
                                            .and_then(|s| s.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        app.task_edit_form = Some(app::TaskEditForm {
                                            task_id: task.id,
                                            title: task.title.clone(),
                                            kind_index,
                                            priority: task.priority.clamp(1, 5) as u8,
                                            spec,
                                            active_field: 0,
                                            cursor: task.title.chars().count(),
                                        });
                                    }
                                } else if app.view == View::Playbooks {
                                    if let Some(pb) =
                                        app.selected_playbook.and_then(|i| app.playbooks.get(i))
                                    {
                                        let steps = app::PlaybookForm::steps_from_json(&pb.steps);
                                        app.playbook_form = Some(app::PlaybookForm {
                                            title: pb.title.clone(),
                                            trigger: pb
                                                .trigger_description
                                                .clone()
                                                .unwrap_or_default(),
                                            tags: pb.tags.join(", "),
                                            steps,
                                            active_field: 0,
                                            cursor: pb.title.chars().count(),
                                            editing_id: Some(pb.id),
                                            selected_step: 0,
                                            editing_step: false,
                                            step_field: 0,
                                        });
                                    }
                                } else if app.view == View::Goals {
                                    if let Some(goal) =
                                        app.selected_goal.and_then(|i| app.goals.get(i))
                                    {
                                        let status_idx = GOAL_STATUSES
                                            .iter()
                                            .position(|s| Some(*s) == goal.status.as_deref())
                                            .unwrap_or(0);
                                        let goal_type_idx = GOAL_TYPES
                                            .iter()
                                            .position(|t| Some(*t) == goal.goal_type.as_deref())
                                            .unwrap_or(0);
                                        app.goal_form = Some(app::GoalForm {
                                            title: goal.title.clone(),
                                            description: goal
                                                .description
                                                .clone()
                                                .unwrap_or_default(),
                                            success_criteria: goal
                                                .success_criteria
                                                .as_ref()
                                                .and_then(|v| {
                                                    v.as_array().and_then(|a| {
                                                        a.first().and_then(|s| {
                                                            s.as_str().map(|s| s.to_string())
                                                        })
                                                    })
                                                })
                                                .unwrap_or_default(),
                                            target_date: goal
                                                .target_date
                                                .clone()
                                                .unwrap_or_default(),
                                            status_index: status_idx,
                                            goal_type_index: goal_type_idx,
                                            priority: goal
                                                .priority
                                                .map(|p| p.to_string())
                                                .unwrap_or_else(|| "0".to_string()),
                                            auto_status: goal.auto_status.unwrap_or(false),
                                            active_field: 0,
                                            cursor: goal.title.chars().count(),
                                            editing_id: Some(goal.id),
                                        });
                                    }
                                } else if app.view == View::Knowledge {
                                    if let Some(k) =
                                        app.selected_knowledge.and_then(|i| app.knowledge.get(i))
                                    {
                                        let cat_idx = KNOWLEDGE_CATEGORIES
                                            .iter()
                                            .position(|c| Some(*c) == k.category.as_deref())
                                            .unwrap_or(0);
                                        app.knowledge_form = Some(app::KnowledgeForm {
                                            title: k.title.clone(),
                                            category_index: cat_idx,
                                            content: k.content.clone().unwrap_or_default(),
                                            tags: k
                                                .tags
                                                .as_ref()
                                                .map(|t| t.join(", "))
                                                .unwrap_or_default(),
                                            active_field: 0,
                                            cursor: k.title.chars().count(),
                                            editing_id: Some(k.id),
                                        });
                                    }
                                } else if app.view == View::Integrations {
                                    if let Some(intg) = app
                                        .selected_integration
                                        .and_then(|i| app.integrations.get(i))
                                    {
                                        let intg_id = intg.id;
                                        let new_enabled = !intg.enabled;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) =
                                                api.toggle_integration(intg_id, new_enabled).await
                                            {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Toggle: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(intgs) = api.list_integrations(pid).await
                                                {
                                                    let _ =
                                                        tx.send(ApiMsg::Integrations(intgs)).await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Webhooks {
                                    if let Some(wh) =
                                        app.selected_webhook.and_then(|i| app.webhooks.get(i))
                                    {
                                        let wh_id = wh.id;
                                        let new_enabled = !wh.enabled;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            let body = serde_json::json!({
                                                "enabled": new_enabled,
                                            });
                                            if let Err(e) = api.update_webhook(wh_id, body).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Toggle: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(whs) = api.list_webhooks(pid).await {
                                                    let _ = tx.send(ApiMsg::Webhooks(whs)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            // h = entity history (Audit view)
                            KeyCode::Char('h') => {
                                if app.view == View::Audit {
                                    if let Some(entry) =
                                        app.selected_audit.and_then(|i| app.audit_log.get(i))
                                    {
                                        if let (Some(etype), Some(eid)) =
                                            (entry.entity_type.clone(), entry.entity_id)
                                        {
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                match api.entity_history(&etype, eid).await {
                                                    Ok(history) => {
                                                        let _ =
                                                            tx.send(ApiMsg::Audit(history)).await;
                                                    }
                                                    Err(e) => {
                                                        let _ = tx
                                                            .send(ApiMsg::Error(format!(
                                                                "History: {e}"
                                                            )))
                                                            .await;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            // D = delete (Decisions, Knowledge, Playbooks, Team, Integrations)
                            KeyCode::Char('D') => {
                                if app.view == View::Decisions {
                                    if let Some(dec) =
                                        app.selected_decision.and_then(|i| app.decisions.get(i))
                                    {
                                        let dec_id = dec.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api.delete_decision(dec_id).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Delete: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(decs) = api.list_decisions(pid).await {
                                                    let _ = tx.send(ApiMsg::Decisions(decs)).await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Knowledge {
                                    if let Some(k) =
                                        app.selected_knowledge.and_then(|i| app.knowledge.get(i))
                                    {
                                        let k_id = k.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api.delete_knowledge(k_id).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Delete: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(knowledge) = api.list_knowledge(pid).await
                                                {
                                                    let _ =
                                                        tx.send(ApiMsg::Knowledge(knowledge)).await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Playbooks {
                                    if let Some(pb) =
                                        app.selected_playbook.and_then(|i| app.playbooks.get(i))
                                    {
                                        let pb_id = pb.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api.delete_playbook(pb_id).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Delete: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(pbs) = api.list_playbooks(pid).await {
                                                    let _ = tx.send(ApiMsg::Playbooks(pbs)).await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Team {
                                    if app.team_focus == 0 {
                                        // Delete role
                                        if let Some(role) =
                                            app.selected_role.and_then(|i| app.roles.get(i))
                                        {
                                            let role_id = role.id;
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                if let Err(e) = api.delete_role(role_id).await {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!("Delete: {e}")))
                                                        .await;
                                                } else if let Ok(roles) = api.list_roles().await {
                                                    let _ = tx.send(ApiMsg::Roles(roles)).await;
                                                }
                                            });
                                        }
                                    } else if app.team_focus == 1 {
                                        // Delete member
                                        if let Some(member) =
                                            app.selected_member.and_then(|i| app.members.get(i))
                                        {
                                            let member_id = member.id;
                                            let api = api.clone();
                                            let tx = tx.clone();
                                            tokio::spawn(async move {
                                                if let Err(e) = api.delete_member(member_id).await {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!("Delete: {e}")))
                                                        .await;
                                                } else if let Ok(members) = api.list_members().await
                                                {
                                                    let _ = tx.send(ApiMsg::Members(members)).await;
                                                }
                                            });
                                        }
                                    }
                                } else if app.view == View::Integrations {
                                    if let Some(intg) = app
                                        .selected_integration
                                        .and_then(|i| app.integrations.get(i))
                                    {
                                        let intg_id = intg.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api.delete_integration(intg_id).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Delete: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(intgs) = api.list_integrations(pid).await
                                                {
                                                    let _ =
                                                        tx.send(ApiMsg::Integrations(intgs)).await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Webhooks {
                                    if let Some(wh) =
                                        app.selected_webhook.and_then(|i| app.webhooks.get(i))
                                    {
                                        let wh_id = wh.id;
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        let pid = app.current_project;
                                        tokio::spawn(async move {
                                            if let Err(e) = api.delete_webhook(wh_id).await {
                                                let _ = tx
                                                    .send(ApiMsg::Error(format!("Delete: {e}")))
                                                    .await;
                                            } else if let Some(pid) = pid {
                                                if let Ok(whs) = api.list_webhooks(pid).await {
                                                    let _ = tx.send(ApiMsg::Webhooks(whs)).await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Reports && app.selected_report.is_some()
                                {
                                    app.modal = Modal::ReportDelete;
                                }
                            }
                            // Logs view controls: T/Y = time range, N/M = limit, B = direction
                            // Playbooks view: T = fetch step templates
                            KeyCode::Char('T') => {
                                if app.view == View::Playbooks {
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.list_step_templates(pid).await {
                                                Ok(templates) => {
                                                    let _ = tx
                                                        .send(ApiMsg::StepTemplates(templates))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::Error(format!(
                                                            "Step templates: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                } else if app.view == View::Logs {
                                    if app.log_time_range_idx > 0 {
                                        app.log_time_range_idx -= 1;
                                    } else {
                                        app.log_time_range_idx = TIME_RANGES.len() - 1;
                                    }
                                }
                            }
                            KeyCode::Char('Y') => {
                                if app.view == View::Logs {
                                    app.log_time_range_idx =
                                        (app.log_time_range_idx + 1) % TIME_RANGES.len();
                                }
                            }
                            KeyCode::Char('N') => {
                                if app.view == View::Logs {
                                    if app.log_limit_idx > 0 {
                                        app.log_limit_idx -= 1;
                                    } else {
                                        app.log_limit_idx = LOG_LIMITS.len() - 1;
                                    }
                                }
                            }
                            KeyCode::Char('M') => {
                                if app.view == View::Logs {
                                    app.log_limit_idx = (app.log_limit_idx + 1) % LOG_LIMITS.len();
                                }
                            }
                            KeyCode::Char('B') => {
                                if app.view == View::Logs {
                                    app.log_direction_idx =
                                        (app.log_direction_idx + 1) % LOG_DIRECTIONS.len();
                                } else {
                                    app.view = View::Source;
                                    app.detail_scroll = 0;
                                    app.source_current_path = String::new();
                                    app.source_blob_content = None;
                                    app.source_blob_path = None;
                                    app.source_blob_scroll = 0;
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            if let Ok(resp) = api.source_tree(pid, "", None).await {
                                                let _ =
                                                    tx.send(ApiMsg::SourceTree(resp.entries)).await;
                                            }
                                        });
                                    }
                                }
                            }
                            // PageUp/PageDown for fast scrolling in Logs
                            KeyCode::PageUp => {
                                if app.view == View::Logs {
                                    app.scroll_logs(-20);
                                }
                            }
                            KeyCode::PageDown => {
                                if app.view == View::Logs {
                                    app.scroll_logs(20);
                                }
                            }
                            // ` = view picker (navigation expansion)
                            KeyCode::Char('`') => {
                                let opts: Vec<String> = ALL_VIEWS
                                    .iter()
                                    .map(|v| format!("[{}] {}", v.shortcut(), v.label()))
                                    .collect();
                                app.transition_options = opts;
                                // Pre-select current view
                                app.transition_selected =
                                    ALL_VIEWS.iter().position(|v| *v == app.view).unwrap_or(0);
                                app.modal = Modal::ViewPicker;
                            }
                            KeyCode::Char('L') => {
                                theme::toggle();
                            }
                            KeyCode::Backspace => {
                                if app.view == View::Source && !app.source_current_path.is_empty() {
                                    let new_path = app
                                        .source_current_path
                                        .rfind('/')
                                        .map(|i| app.source_current_path[..i].to_string())
                                        .unwrap_or_default();
                                    app.source_current_path = new_path.clone();
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            if let Ok(resp) =
                                                api.source_tree(pid, &new_path, None).await
                                            {
                                                let _ =
                                                    tx.send(ApiMsg::SourceTree(resp.entries)).await;
                                            }
                                        });
                                    }
                                }
                            }
                            KeyCode::Char('P') => {
                                if app.view == View::Git {
                                    if let Some(pid) = app.current_project {
                                        let api = api.clone();
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            match api.push_main(pid).await {
                                                Ok(resp) => {
                                                    let _ = tx
                                                        .send(ApiMsg::GitActionResult(resp.message))
                                                        .await;
                                                }
                                                Err(e) => {
                                                    let _ = tx
                                                        .send(ApiMsg::GitActionResult(format!(
                                                            "Push main error: {e}"
                                                        )))
                                                        .await;
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                            _ => {}
                        },
                    }
                }

                // When task selection changes, mark for debounced fetch
                if app.modal == Modal::None
                    && app.view == View::Tasks
                    && matches!(
                        key.code,
                        KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
                    )
                {
                    last_nav_time = Some(Instant::now());
                }

                // Fetch goal progress on selection change
                if app.modal == Modal::None
                    && app.view == View::Goals
                    && matches!(
                        key.code,
                        KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
                    )
                {
                    if let Some(goal) = app.selected_goal.and_then(|i| app.goals.get(i)) {
                        let gid = goal.id;
                        {
                            let api = api.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                if let Ok(progress) = api.get_goal_progress(gid).await {
                                    let _ = tx.send(ApiMsg::GoalProgress(progress)).await;
                                }
                            });
                        }
                        {
                            let api = api.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                if let Ok(stats) = api.get_goal_stats(gid).await {
                                    let _ = tx.send(ApiMsg::GoalStats(stats)).await;
                                }
                                if let Ok(children) = api.list_goal_children(gid).await {
                                    let _ = tx.send(ApiMsg::GoalChildren(children)).await;
                                }
                            });
                        }
                        {
                            let api = api.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                if let Ok(tasks) = api.list_goal_tasks(gid, 50, 0).await {
                                    let _ = tx.send(ApiMsg::GoalTasksList(tasks)).await;
                                }
                            });
                        }
                        {
                            let api = api.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                if let Ok(comments) = api.list_goal_comments(gid).await {
                                    let _ = tx.send(ApiMsg::GoalComments(comments)).await;
                                }
                            });
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
