use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // List
    let block = Block::default()
        .title(" Goals ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::green()));

    let items: Vec<ListItem> = app
        .goals
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let status = g.status.as_deref().unwrap_or("active");
            let goal_type = g.goal_type.as_deref().unwrap_or("epic");
            let priority = g.priority.unwrap_or(0);
            let color = match status {
                "active" => theme::green(),
                "achieved" => theme::blue(),
                "paused" => theme::yellow(),
                "abandoned" => theme::red(),
                _ => theme::text(),
            };
            let style = if Some(i) == app.selected_goal {
                Style::default().fg(theme::base()).bg(theme::green())
            } else {
                Style::default().fg(color)
            };
            let prio_str = if priority != 0 {
                format!(" P:{}", priority)
            } else {
                String::new()
            };
            ListItem::new(Line::styled(
                format!(" [{}:{}]{} {}", goal_type, status, prio_str, g.title),
                style,
            ))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Goal Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_goal
        .and_then(|i| app.goals.get(i))
        .map(|g| {
            let status = g.status.as_deref().unwrap_or("active");
            let goal_type = g.goal_type.as_deref().unwrap_or("epic");
            let priority = g.priority.unwrap_or(0);
            let auto_status = g.auto_status.unwrap_or(false);
            let status_color = match status {
                "active" => theme::green(),
                "achieved" => theme::blue(),
                "paused" => theme::yellow(),
                "abandoned" => theme::red(),
                _ => theme::text(),
            };
            let mut lines = vec![
                Line::styled(&g.title, Style::default().fg(theme::green())),
                Line::styled(
                    format!(
                        "Type: {}  Status: {}  Priority: {}",
                        goal_type, status, priority
                    ),
                    Style::default().fg(status_color),
                ),
            ];

            if auto_status {
                lines.push(Line::styled(
                    "Auto-status: ON (derived from tasks)",
                    Style::default().fg(theme::peach()),
                ));
            }

            if g.parent_goal_id.is_some() {
                lines.push(Line::styled(
                    "Has parent goal",
                    Style::default().fg(theme::subtext0()),
                ));
            }

            if let Some(ref desc) = g.description {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    desc.as_str(),
                    Style::default().fg(theme::text()),
                ));
            }

            if let Some(ref date) = g.target_date {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Target: {}", date),
                    Style::default().fg(theme::subtext0()),
                ));
            }

            if let Some(ref criteria) = g.success_criteria {
                if !criteria.is_null() {
                    lines.push(Line::from(""));
                    lines.push(Line::styled(
                        "Success Criteria:",
                        Style::default().fg(theme::peach()),
                    ));
                    let text = if let Some(s) = criteria.as_str() {
                        s.to_string()
                    } else {
                        serde_json::to_string_pretty(criteria).unwrap_or_default()
                    };
                    for line in text.lines() {
                        lines.push(Line::styled(
                            line.to_string(),
                            Style::default().fg(theme::text()),
                        ));
                    }
                }
            }

            // Progress bar
            if let Some(ref progress) = app.goal_progress {
                if progress.goal_id == g.id {
                    lines.push(Line::from(""));
                    let total = progress.total_tasks;
                    let done = progress.done_tasks;
                    let pct = if total > 0 {
                        (done as f64 / total as f64 * 100.0) as u32
                    } else {
                        0
                    };
                    let bar_width = 20usize;
                    let filled = if total > 0 {
                        (done as usize * bar_width / total as usize).min(bar_width)
                    } else {
                        0
                    };
                    let bar: String = format!(
                        "[{}{}] {}/{}  {}%",
                        "=".repeat(filled),
                        " ".repeat(bar_width - filled),
                        done,
                        total,
                        pct
                    );
                    lines.push(Line::styled(
                        format!("Progress: {}", bar),
                        Style::default().fg(theme::blue()),
                    ));
                }
            }

            // Stats
            if let Some(ref stats) = app.goal_stats {
                if stats.goal_id == g.id {
                    lines.push(Line::from(""));
                    lines.push(Line::styled(
                        "Task Stats:",
                        Style::default().fg(theme::peach()),
                    ));
                    lines.push(Line::styled(
                        format!(
                            "  Backlog: {}  Ready: {}  Working: {}  Done: {}  Cancelled: {}",
                            stats.backlog_count,
                            stats.ready_count,
                            stats.working_count,
                            stats.done_count,
                            stats.cancelled_count,
                        ),
                        Style::default().fg(theme::text()),
                    ));
                    if stats.blocked_count > 0 {
                        lines.push(Line::styled(
                            format!("  Blocked: {}", stats.blocked_count),
                            Style::default().fg(theme::red()),
                        ));
                    }
                    if stats.total_cost_usd > 0.0 {
                        lines.push(Line::styled(
                            format!("  Cost: ${:.4}", stats.total_cost_usd),
                            Style::default().fg(theme::subtext0()),
                        ));
                    }
                    if let Some(avg) = stats.avg_completion_hours {
                        lines.push(Line::styled(
                            format!("  Avg completion: {:.1}h", avg),
                            Style::default().fg(theme::subtext0()),
                        ));
                    }
                }
            }

            // Linked tasks
            if !app.goal_tasks.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Linked Tasks ({}):", app.goal_tasks.len()),
                    Style::default().fg(theme::peach()),
                ));
                for t in &app.goal_tasks {
                    let state_color = match t.state.as_str() {
                        "done" => theme::green(),
                        "cancelled" => theme::red(),
                        "ready" => theme::blue(),
                        _ => theme::text(),
                    };
                    lines.push(Line::styled(
                        format!("  #{} [{}] {}", t.number, t.state, t.title),
                        Style::default().fg(state_color),
                    ));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::styled(
                "[l] Link tasks",
                Style::default().fg(theme::overlay0()),
            ));

            // Children
            if !app.goal_children.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Children ({}):", app.goal_children.len()),
                    Style::default().fg(theme::peach()),
                ));
                for child in &app.goal_children {
                    let child_type = child.goal_type.as_deref().unwrap_or("epic");
                    let child_status = child.status.as_deref().unwrap_or("active");
                    lines.push(Line::styled(
                        format!("  [{}:{}] {}", child_type, child_status, child.title),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a goal",
                Style::default().fg(theme::overlay0()),
            )]
        });

    let content_len = content.len() as u16;
    let inner_height = chunks[1].height.saturating_sub(2);
    if content_len <= inner_height {
        app.detail_scroll = 0;
    } else if app.detail_scroll > content_len.saturating_sub(inner_height) {
        app.detail_scroll = content_len.saturating_sub(inner_height);
    }

    f.render_widget(
        Paragraph::new(content)
            .block(detail_block)
            .wrap(Wrap { trim: true })
            .scroll((app.detail_scroll, 0)),
        chunks[1],
    );
}
