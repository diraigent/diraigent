use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

fn entity_color(entity_type: &str) -> ratatui::style::Color {
    match entity_type {
        "task" => theme::blue(),
        "work" => theme::green(),
        "decision" => theme::mauve(),
        "observation" => theme::peach(),
        "member" => theme::teal(),
        "role" => theme::yellow(),
        _ => theme::text(),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // List
    let block = Block::default()
        .title(" Audit Log ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .audit_log
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let time = entry
                .created_at
                .as_deref()
                .and_then(|s| s.get(11..16))
                .unwrap_or("??:??");
            let action = entry.action.as_deref().unwrap_or("?");
            let summary = entry.summary.as_deref().unwrap_or("");
            let etype = entry.entity_type.as_deref().unwrap_or("");
            let color = entity_color(etype);

            let label = format!(" {} [{}] {}", time, action, truncate(summary, 40));
            let style = if Some(i) == app.selected_audit {
                Style::default().fg(theme::base()).bg(theme::yellow())
            } else {
                Style::default().fg(color)
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let content = app
        .selected_audit
        .and_then(|i| app.audit_log.get(i))
        .map(|entry| {
            let mut lines = vec![];

            let action = entry.action.as_deref().unwrap_or("—");
            let etype = entry.entity_type.as_deref().unwrap_or("—");
            let color = entity_color(etype);

            lines.push(Line::styled(
                format!("Action: {}", action),
                Style::default().fg(theme::text()),
            ));
            lines.push(Line::styled(
                format!(
                    "Entity: {} {}",
                    etype,
                    entry
                        .entity_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "—".into())
                ),
                Style::default().fg(color),
            ));

            // Actor
            let actor_name = entry
                .actor_agent_id
                .and_then(|aid| app.agents.iter().find(|a| a.id == aid))
                .map(|a| a.name.as_str())
                .unwrap_or("system");
            lines.push(Line::styled(
                format!("Actor: {}", actor_name),
                Style::default().fg(theme::mauve()),
            ));

            if let Some(ts) = &entry.created_at {
                lines.push(Line::styled(
                    format!("Time: {}", ts.get(..19).unwrap_or(ts)),
                    Style::default().fg(theme::subtext0()),
                ));
            }

            lines.push(Line::from(""));

            if let Some(summary) = &entry.summary {
                lines.push(Line::styled("Summary:", Style::default().fg(theme::blue())));
                for line in summary.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::text()),
                    ));
                }
                lines.push(Line::from(""));
            }

            // Before/after state
            if let Some(before) = &entry.before_state {
                lines.push(Line::styled("Before:", Style::default().fg(theme::red())));
                let pretty = serde_json::to_string_pretty(before).unwrap_or_default();
                for line in pretty.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::subtext0()),
                    ));
                }
                lines.push(Line::from(""));
            }

            if let Some(after) = &entry.after_state {
                lines.push(Line::styled("After:", Style::default().fg(theme::green())));
                let pretty = serde_json::to_string_pretty(after).unwrap_or_default();
                for line in pretty.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::subtext0()),
                    ));
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select an audit entry",
                Style::default().fg(theme::overlay0()),
            )]
        });

    // Clamp scroll
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

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", truncated)
    } else {
        truncated
    }
}
