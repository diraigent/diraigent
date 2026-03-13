use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

fn kind_color(kind: &str) -> ratatui::style::Color {
    match kind {
        "ci" => theme::blue(),
        "deploy" => theme::green(),
        "error" => theme::red(),
        "merge" => theme::mauve(),
        "release" => theme::teal(),
        "alert" => theme::yellow(),
        "custom" => theme::peach(),
        _ => theme::text(),
    }
}

fn severity_color(severity: &str) -> ratatui::style::Color {
    match severity {
        "info" => theme::blue(),
        "warning" => theme::yellow(),
        "error" => theme::red(),
        "critical" => theme::mauve(),
        _ => theme::text(),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Build filter label
    let mut title = String::from(" Events ");
    let kind_f = app.event_kind_filter.as_deref();
    let sev_f = app.event_severity_filter.as_deref();
    if kind_f.is_some() || sev_f.is_some() {
        title.push('[');
        if let Some(k) = kind_f {
            title.push_str(k);
        }
        if kind_f.is_some() && sev_f.is_some() {
            title.push_str(", ");
        }
        if let Some(s) = sev_f {
            title.push_str(s);
        }
        title.push_str("] ");
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let filtered = app.filtered_events();
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            let time = ev
                .created_at
                .as_deref()
                .and_then(|s| s.get(11..16))
                .unwrap_or("??:??");
            let sev_col = severity_color(&ev.severity);
            let k_col = kind_color(&ev.kind);

            let selected = Some(i) == app.selected_event;
            if selected {
                ListItem::new(Line::styled(
                    format!(
                        " {} [{}] [{}] {}",
                        time,
                        ev.kind,
                        ev.severity,
                        truncate(&ev.title, 40)
                    ),
                    Style::default().fg(theme::base()).bg(theme::yellow()),
                ))
            } else {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", time),
                        Style::default().fg(theme::subtext0()),
                    ),
                    Span::styled(format!("[{}]", ev.kind), Style::default().fg(k_col)),
                    Span::styled(format!(" [{}] ", ev.severity), Style::default().fg(sev_col)),
                    Span::styled(truncate(&ev.title, 40), Style::default().fg(theme::text())),
                ]))
            }
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Event Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    // Get the selected event data (clone to avoid borrow conflict with app)
    let selected_ev = app.selected_event.and_then(|i| {
        let filtered = app.filtered_events();
        filtered.get(i).cloned().cloned()
    });

    // Resolve agent/task names while we have the borrow
    let agent_name = selected_ev
        .as_ref()
        .and_then(|ev| ev.agent_id)
        .and_then(|aid| app.agents.iter().find(|a| a.id == aid))
        .map(|a| a.name.clone())
        .unwrap_or_else(|| "—".to_string());

    let task_label = selected_ev
        .as_ref()
        .and_then(|ev| ev.related_task_id)
        .map(|tid| {
            app.tasks
                .iter()
                .find(|t| t.id == tid)
                .map(|t| format!("#{} {}", t.number, t.title))
                .unwrap_or_else(|| tid.to_string()[..13].to_string())
        });

    let content = selected_ev
        .map(|ev| {
            let mut lines = vec![
                Line::styled(ev.title.clone(), Style::default().fg(theme::text())),
                Line::from(""),
                // Kind + Severity
                Line::from(vec![
                    Span::styled("Kind: ", Style::default().fg(theme::subtext0())),
                    Span::styled(ev.kind.clone(), Style::default().fg(kind_color(&ev.kind))),
                    Span::styled("  Severity: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        ev.severity.clone(),
                        Style::default().fg(severity_color(&ev.severity)),
                    ),
                ]),
                // Source
                Line::from(vec![
                    Span::styled("Source: ", Style::default().fg(theme::subtext0())),
                    Span::styled(ev.source.clone(), Style::default().fg(theme::blue())),
                ]),
                // Agent
                Line::from(vec![
                    Span::styled("Agent: ", Style::default().fg(theme::subtext0())),
                    Span::styled(agent_name.clone(), Style::default().fg(theme::mauve())),
                ]),
            ];

            // Related task
            if let Some(ref label) = task_label {
                lines.push(Line::from(vec![
                    Span::styled("Task: ", Style::default().fg(theme::subtext0())),
                    Span::styled(label.clone(), Style::default().fg(theme::blue())),
                ]));
            }

            // Timestamp
            if let Some(ref ts) = ev.created_at {
                lines.push(Line::from(vec![
                    Span::styled("Time: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        ts.get(..19).unwrap_or(ts).to_string(),
                        Style::default().fg(theme::subtext0()),
                    ),
                ]));
            }

            // Description
            if let Some(ref desc) = ev.description {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    "Description:".to_string(),
                    Style::default().fg(theme::green()),
                ));
                for line in desc.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            // Metadata
            if ev.metadata != serde_json::Value::Null
                && ev.metadata != serde_json::Value::Object(serde_json::Map::new())
            {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    "Metadata:".to_string(),
                    Style::default().fg(theme::teal()),
                ));
                let pretty = serde_json::to_string_pretty(&ev.metadata).unwrap_or_default();
                for line in pretty.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::subtext0()),
                    ));
                }
            }

            // Actions hint
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Keys: [n] New  [f] Filter kind  [F] Filter severity  [Esc] Clear filters"
                    .to_string(),
                Style::default().fg(theme::overlay0()),
            ));

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select an event",
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
