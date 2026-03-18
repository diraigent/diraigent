use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::client::Work;
use crate::theme;

/// Map API work status to user-friendly display name.
pub fn work_status_label(status: &str) -> &str {
    match status {
        "achieved" => "Done",
        "paused" => "Pause",
        "abandoned" => "Abandon",
        other => other,
    }
}

fn make_work_item(
    app: &App,
    g: &Work,
    i: usize,
    selected: Option<usize>,
    is_focused_section: bool,
) -> ListItem<'static> {
    let status = g.status.as_deref().unwrap_or("active");
    let work_type = g.work_type.as_deref().unwrap_or("epic");
    let priority = g.priority.unwrap_or(0);
    let color = match status {
        "active" => theme::green(),
        "achieved" => theme::blue(),
        "paused" => theme::yellow(),
        "abandoned" => theme::red(),
        _ => theme::text(),
    };
    let style = if is_focused_section && Some(i) == selected {
        Style::default().fg(theme::base()).bg(theme::green())
    } else {
        Style::default().fg(color)
    };
    let prio_str = if priority != 0 {
        format!(" P:{}", priority)
    } else {
        String::new()
    };
    let progress_str = app
        .work_progress_map
        .get(&g.id)
        .map(|p| format!(" ({}/{})", p.done_tasks, p.total_tasks))
        .unwrap_or_default();
    ListItem::new(Line::styled(
        format!(
            " [{}:{}]{} {}{}",
            work_type,
            work_status_label(status),
            prio_str,
            g.title,
            progress_str
        ),
        style,
    ))
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Split left panel into active (top) and done (bottom) sections
    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[0]);

    let active_focused = app.work_section == 0;
    let done_focused = app.work_section == 1;

    // Active/Paused list (top)
    {
        let border_color = if active_focused {
            theme::green()
        } else {
            theme::surface1()
        };
        let block = Block::default()
            .title(" Active ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let items: Vec<ListItem> = app
            .work_items
            .iter()
            .enumerate()
            .map(|(i, g)| make_work_item(app, g, i, app.selected_work, active_focused))
            .collect();

        app.work_list_state.select(app.selected_work);
        f.render_stateful_widget(
            List::new(items).block(block),
            list_chunks[0],
            &mut app.work_list_state,
        );
    }

    // Done/Abandoned list (bottom)
    {
        let border_color = if done_focused {
            theme::green()
        } else {
            theme::surface1()
        };
        let title = if app.done_work_has_more || app.done_work_page > 0 {
            format!(" Done (p{}) ", app.done_work_page + 1)
        } else {
            " Done ".to_string()
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let items: Vec<ListItem> = app
            .done_work_items
            .iter()
            .enumerate()
            .map(|(i, g)| make_work_item(app, g, i, app.selected_done_work, done_focused))
            .collect();

        app.done_work_list_state.select(app.selected_done_work);
        f.render_stateful_widget(
            List::new(items).block(block),
            list_chunks[1],
            &mut app.done_work_list_state,
        );
    }

    // Detail panel (right)
    let detail_block = Block::default()
        .title(" Work Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    // Get selected work item from the appropriate section
    let selected_goal = if app.work_section == 0 {
        app.selected_work.and_then(|i| app.work_items.get(i))
    } else {
        app.selected_done_work
            .and_then(|i| app.done_work_items.get(i))
    };

    let content = selected_goal
        .map(|g| {
            let status = g.status.as_deref().unwrap_or("active");
            let work_type = g.work_type.as_deref().unwrap_or("epic");
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
                Line::styled(g.title.clone(), Style::default().fg(theme::green())),
                Line::styled(
                    format!(
                        "Type: {}  Status: {}  Priority: {}",
                        work_type,
                        work_status_label(status),
                        priority
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

            if g.parent_work_id.is_some() {
                lines.push(Line::styled(
                    "Has parent work item",
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
            if let Some(ref progress) = app.work_progress {
                if progress.work_id == g.id {
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
            if let Some(ref stats) = app.work_stats {
                if stats.work_id == g.id {
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
            if !app.work_tasks.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Linked Tasks ({}):", app.work_tasks.len()),
                    Style::default().fg(theme::peach()),
                ));
                for t in &app.work_tasks {
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
                "[t] New task  [c] Comment  [</> page]",
                Style::default().fg(theme::overlay0()),
            ));

            // Comments
            if !app.work_comments.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Comments ({}):", app.work_comments.len()),
                    Style::default().fg(theme::peach()),
                ));
                for gc in &app.work_comments {
                    let time = gc
                        .created_at
                        .as_deref()
                        .and_then(|s| s.get(11..16))
                        .unwrap_or("??:??");
                    let author = if gc.agent_id.is_some() {
                        "agent"
                    } else {
                        "human"
                    };
                    lines.push(Line::from(vec![
                        ratatui::text::Span::styled(
                            format!("{} ", time),
                            Style::default().fg(theme::overlay0()),
                        ),
                        ratatui::text::Span::styled(
                            format!("[{}] ", author),
                            Style::default().fg(theme::mauve()),
                        ),
                        ratatui::text::Span::styled(
                            gc.content.clone(),
                            Style::default().fg(theme::text()),
                        ),
                    ]));
                }
            }

            // Children
            if !app.work_children.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Children ({}):", app.work_children.len()),
                    Style::default().fg(theme::peach()),
                ));
                for child in &app.work_children {
                    let work_type = child.work_type.as_deref().unwrap_or("epic");
                    let child_status = child.status.as_deref().unwrap_or("active");
                    lines.push(Line::styled(
                        format!(
                            "  [{}:{}] {}",
                            work_type,
                            work_status_label(child_status),
                            child.title
                        ),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a work item",
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
