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

    // ── Left panel: Plan list ──
    let block = Block::default()
        .title(" Plans ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::green()));

    let items: Vec<ListItem> = app
        .plans
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let status_color = match p.status.as_str() {
                "active" => theme::green(),
                "completed" => theme::blue(),
                "archived" => theme::overlay0(),
                "cancelled" => theme::red(),
                _ => theme::text(),
            };
            let style = if Some(i) == app.selected_plan {
                Style::default().fg(theme::base()).bg(theme::green())
            } else {
                Style::default().fg(status_color)
            };
            ListItem::new(Line::styled(format!(" [{}] {}", p.status, p.title), style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // ── Right panel: Plan detail ──
    let detail_block = Block::default()
        .title(" Plan Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_plan
        .and_then(|i| app.plans.get(i))
        .map(|p| {
            let status_color = match p.status.as_str() {
                "active" => theme::green(),
                "completed" => theme::blue(),
                "archived" => theme::overlay0(),
                "cancelled" => theme::red(),
                _ => theme::text(),
            };

            let mut lines = vec![
                Line::styled(&p.title, Style::default().fg(theme::green())),
                Line::styled(
                    format!("Status: {}", p.status),
                    Style::default().fg(status_color),
                ),
            ];

            if let Some(ref desc) = p.description {
                if !desc.is_empty() {
                    lines.push(Line::from(""));
                    for l in desc.lines() {
                        lines.push(Line::styled(
                            l.to_string(),
                            Style::default().fg(theme::text()),
                        ));
                    }
                }
            }

            if let Some(ref created) = p.created_at {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Created: {}", &created[..created.len().min(19)]),
                    Style::default().fg(theme::subtext0()),
                ));
            }

            // Progress bar
            if let Some(ref progress) = app.plan_progress {
                if progress.plan_id == p.id {
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
                    let bar = format!(
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

                    if progress.working_tasks > 0 {
                        lines.push(Line::styled(
                            format!("  Working: {}", progress.working_tasks),
                            Style::default().fg(theme::peach()),
                        ));
                    }
                    if progress.cancelled_tasks > 0 {
                        lines.push(Line::styled(
                            format!("  Cancelled: {}", progress.cancelled_tasks),
                            Style::default().fg(theme::red()),
                        ));
                    }
                }
            }

            // Task list
            if !app.plan_tasks.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Tasks ({}):", app.plan_tasks.len()),
                    Style::default().fg(theme::peach()),
                ));
                for t in &app.plan_tasks {
                    let state_color = match t.state.as_str() {
                        "done" => theme::green(),
                        "cancelled" => theme::red(),
                        "ready" => theme::blue(),
                        "backlog" => theme::overlay0(),
                        _ => theme::yellow(),
                    };
                    lines.push(Line::styled(
                        format!("  #{} [{}] {}", t.number, t.state, t.title),
                        Style::default().fg(state_color),
                    ));
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a plan",
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
