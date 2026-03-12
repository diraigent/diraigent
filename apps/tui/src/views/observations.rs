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
        .title(" Observations ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::peach()));

    let items: Vec<ListItem> = app
        .observations
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let severity = o.severity.as_deref().unwrap_or("info");
            let kind = o.kind.as_deref().unwrap_or("—");
            let color = match severity {
                "critical" | "high" => theme::red(),
                "medium" => theme::yellow(),
                _ => theme::text(),
            };
            let style = if Some(i) == app.selected_observation {
                Style::default().fg(theme::base()).bg(theme::peach())
            } else {
                Style::default().fg(color)
            };
            ListItem::new(Line::styled(
                format!(" [{}] [{}] {}", severity, kind, o.title),
                style,
            ))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Observation Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_observation
        .and_then(|i| app.observations.get(i))
        .map(|o| {
            let severity = o.severity.as_deref().unwrap_or("info");
            let kind = o.kind.as_deref().unwrap_or("—");
            let status = o.status.as_deref().unwrap_or("open");
            let severity_color = match severity {
                "critical" | "high" => theme::red(),
                "medium" => theme::yellow(),
                _ => theme::text(),
            };
            let mut lines = vec![
                Line::styled(&o.title, Style::default().fg(theme::peach())),
                Line::styled(
                    format!("Status: {}  Severity: {}  Kind: {}", status, severity, kind),
                    Style::default().fg(severity_color),
                ),
            ];

            if let Some(ref source) = o.source {
                let source_line = if let Some(ref task_id) = o.source_task_id {
                    format!("Source: {}  Task: {}", source, &task_id.to_string()[..13])
                } else {
                    format!("Source: {}", source)
                };
                lines.push(Line::styled(
                    source_line,
                    Style::default().fg(theme::blue()),
                ));
            }

            if let Some(ref desc) = o.description {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    desc.as_str(),
                    Style::default().fg(theme::text()),
                ));
            }

            // Show available actions hint
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Actions: [s] Status  [d] Dismiss  [p] Promote",
                Style::default().fg(theme::overlay0()),
            ));

            if let Some(ref evidence) = o.evidence {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    "Evidence:",
                    Style::default().fg(theme::teal()),
                ));
                let formatted = serde_json::to_string_pretty(evidence).unwrap_or_default();
                for line in formatted.lines() {
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
                " Select an observation",
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
