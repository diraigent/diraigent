use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    // Split into: header (3), metrics row (5), bottom half split into events + tasks
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Project summary header
            Constraint::Length(7), // Key metrics
            Constraint::Min(10),   // Bottom: events + tasks
        ])
        .split(area);

    // ── Project Summary Header ──────────────────────────────────
    render_header(f, main_chunks[0], app);

    // ── Key Metrics ─────────────────────────────────────────────
    render_metrics(f, main_chunks[1], app);

    // ── Bottom half: events (left) + active tasks (right) ───────
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[2]);

    render_events(f, bottom_chunks[0], app);
    render_active_tasks(f, bottom_chunks[1], app);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Project Overview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::blue()));

    let project = app
        .current_project
        .and_then(|pid| app.projects.iter().find(|p| p.id == pid));

    let lines = if let Some(p) = project {
        let repo = p.repo_url.as_deref().unwrap_or("–");
        let branch = p.default_branch.as_deref().unwrap_or("main");
        vec![
            Line::from(vec![
                Span::styled("  Project: ", Style::default().fg(theme::subtext0())),
                Span::styled(&p.name, Style::default().fg(theme::green())),
                Span::styled("  (", Style::default().fg(theme::overlay0())),
                Span::styled(&p.slug, Style::default().fg(theme::teal())),
                Span::styled(")", Style::default().fg(theme::overlay0())),
            ]),
            Line::from(vec![
                Span::styled("  Repo:    ", Style::default().fg(theme::subtext0())),
                Span::styled(repo, Style::default().fg(theme::text())),
                Span::styled("  Branch: ", Style::default().fg(theme::subtext0())),
                Span::styled(branch, Style::default().fg(theme::mauve())),
            ]),
            Line::from(vec![Span::styled(
                format!(
                    "  Agents:  {} online",
                    app.agents.iter().filter(|a| a.status != "offline").count()
                ),
                Style::default().fg(theme::text()),
            )]),
        ]
    } else {
        vec![Line::styled(
            "  No project selected",
            Style::default().fg(theme::overlay0()),
        )]
    };

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_metrics(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Metrics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::peach()));

    if let Some(ref metrics) = app.dashboard_metrics {
        let ts = metrics.task_summary.clone().unwrap_or_default();
        let cs = metrics.cost_summary.clone().unwrap_or_default();
        let ts = &ts;
        let cs = &cs;
        let active_agents = metrics
            .agent_breakdown
            .iter()
            .filter(|a| a.tasks_in_progress > 0)
            .count();

        let lines = vec![
            Line::from(vec![
                Span::styled("  Total Cost: ", Style::default().fg(theme::subtext0())),
                Span::styled(
                    format!("${:.2}", cs.total_cost_usd),
                    Style::default().fg(theme::green()),
                ),
                Span::styled("    Tokens: ", Style::default().fg(theme::subtext0())),
                Span::styled(
                    format!(
                        "{}in / {}out",
                        format_tokens(cs.total_input_tokens),
                        format_tokens(cs.total_output_tokens)
                    ),
                    Style::default().fg(theme::text()),
                ),
                Span::styled(
                    "    Active Agents: ",
                    Style::default().fg(theme::subtext0()),
                ),
                Span::styled(
                    format!("{}", active_agents),
                    Style::default().fg(theme::blue()),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Tasks: ", Style::default().fg(theme::subtext0())),
                Span::styled(
                    format!("{} total", ts.total),
                    Style::default().fg(theme::text()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{} ready", ts.ready),
                    Style::default().fg(theme::blue()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{} working", ts.in_progress),
                    Style::default().fg(theme::yellow()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{} done", ts.done),
                    Style::default().fg(theme::green()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{} cancelled", ts.cancelled),
                    Style::default().fg(theme::red()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{} backlog", ts.backlog),
                    Style::default().fg(theme::overlay0()),
                ),
            ]),
        ];

        f.render_widget(Paragraph::new(lines).block(block), area);
    } else {
        // Show metrics from task list as fallback
        let total = app.tasks.len();
        let ready = app.tasks.iter().filter(|t| t.state == "ready").count();
        let working = app
            .tasks
            .iter()
            .filter(|t| !matches!(t.state.as_str(), "ready" | "done" | "cancelled" | "backlog"))
            .count();
        let done = app.tasks.iter().filter(|t| t.state == "done").count();
        let cancelled = app.tasks.iter().filter(|t| t.state == "cancelled").count();
        let backlog = app.tasks.iter().filter(|t| t.state == "backlog").count();

        let lines = vec![
            Line::styled(
                "  Loading metrics...",
                Style::default().fg(theme::overlay0()),
            ),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Tasks: ", Style::default().fg(theme::subtext0())),
                Span::styled(format!("{total} total"), Style::default().fg(theme::text())),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(format!("{ready} ready"), Style::default().fg(theme::blue())),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{working} working"),
                    Style::default().fg(theme::yellow()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(format!("{done} done"), Style::default().fg(theme::green())),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{cancelled} cancelled"),
                    Style::default().fg(theme::red()),
                ),
                Span::styled("  │  ", Style::default().fg(theme::surface1())),
                Span::styled(
                    format!("{backlog} backlog"),
                    Style::default().fg(theme::overlay0()),
                ),
            ]),
        ];

        f.render_widget(Paragraph::new(lines).block(block), area);
    }
}

fn render_events(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Recent Events ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::teal()));

    if app.dashboard_events.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                " No recent events",
                Style::default().fg(theme::overlay0()),
            ))
            .block(block),
            area,
        );
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for ev in app.dashboard_events.iter().take(15) {
        let severity_color = match ev.severity.as_str() {
            "critical" | "high" => theme::red(),
            "medium" => theme::yellow(),
            "low" => theme::blue(),
            _ => theme::subtext0(),
        };

        let time_str = ev
            .created_at
            .as_deref()
            .and_then(|s| s.get(11..16))
            .unwrap_or("--:--");

        let agent_name = ev
            .agent_id
            .and_then(|aid| app.agents.iter().find(|a| a.id == aid))
            .map(|a| a.name.as_str())
            .unwrap_or("");

        let agent_suffix = if agent_name.is_empty() {
            String::new()
        } else {
            format!(" ({})", agent_name)
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", time_str),
                Style::default().fg(theme::overlay0()),
            ),
            Span::styled(
                format!("[{}] ", &ev.kind),
                Style::default().fg(severity_color),
            ),
            Span::styled(&ev.title, Style::default().fg(theme::text())),
            Span::styled(agent_suffix, Style::default().fg(theme::subtext0())),
        ]));
    }

    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_active_tasks(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Active Tasks ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::yellow()));

    let active_tasks: Vec<&crate::client::Task> = app
        .tasks
        .iter()
        .filter(|t| !matches!(t.state.as_str(), "done" | "cancelled"))
        .collect();

    if active_tasks.is_empty() {
        f.render_widget(
            Paragraph::new(Line::styled(
                " No active tasks",
                Style::default().fg(theme::overlay0()),
            ))
            .block(block),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from(" #").style(Style::default().fg(theme::overlay0())),
        Cell::from("Title").style(Style::default().fg(theme::overlay0())),
        Cell::from("State").style(Style::default().fg(theme::overlay0())),
        Cell::from("Pri").style(Style::default().fg(theme::overlay0())),
        Cell::from("Agent").style(Style::default().fg(theme::overlay0())),
        Cell::from("Cost").style(Style::default().fg(theme::overlay0())),
    ]);

    let rows: Vec<Row> = active_tasks
        .iter()
        .map(|t| {
            let state_color = match t.state.as_str() {
                "ready" => theme::blue(),
                "backlog" => theme::overlay0(),
                "human_review" => theme::mauve(),
                _ => theme::yellow(), // working/implement/review/etc
            };

            let agent_name = t
                .assigned_agent_id
                .and_then(|aid| app.agents.iter().find(|a| a.id == aid))
                .map(|a| a.name.clone())
                .unwrap_or_default();

            // Truncate title to fit
            let title = if t.title.len() > 40 {
                format!("{}…", &t.title[..39])
            } else {
                t.title.clone()
            };

            Row::new(vec![
                Cell::from(format!(" {}", t.number)).style(Style::default().fg(theme::subtext0())),
                Cell::from(title).style(Style::default().fg(theme::text())),
                Cell::from(t.state.clone()).style(Style::default().fg(state_color)),
                Cell::from(format!("{}", t.priority)).style(Style::default().fg(theme::text())),
                Cell::from(agent_name).style(Style::default().fg(theme::subtext0())),
                Cell::from(format!("${:.2}", t.cost_usd))
                    .style(Style::default().fg(theme::green())),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(block);

    f.render_widget(table, area);
}

fn format_tokens(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}
