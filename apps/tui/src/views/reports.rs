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

    // Left panel: report list
    let block = Block::default()
        .title(" Reports ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::mauve()));

    let items: Vec<ListItem> = app
        .reports
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let status_color = match r.status.as_str() {
                "completed" => theme::green(),
                "in_progress" => theme::yellow(),
                "pending" => theme::blue(),
                "failed" => theme::red(),
                _ => theme::text(),
            };
            let style = if Some(i) == app.selected_report {
                Style::default().fg(theme::base()).bg(theme::mauve())
            } else {
                Style::default().fg(status_color)
            };
            let created = r
                .created_at
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("—");
            ListItem::new(Line::styled(
                format!(" [{}] [{}] {} ({})", r.status, r.kind, r.title, created),
                style,
            ))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Right panel: report detail
    let detail_block = Block::default()
        .title(" Report Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_report
        .and_then(|i| app.reports.get(i))
        .map(|r| {
            let mut lines = vec![
                Line::styled(&r.title, Style::default().fg(theme::mauve())),
                Line::styled(
                    format!("Kind: {}", r.kind),
                    Style::default().fg(theme::blue()),
                ),
                Line::styled(
                    format!("Status: {}", r.status),
                    Style::default().fg(match r.status.as_str() {
                        "completed" => theme::green(),
                        "in_progress" => theme::yellow(),
                        "pending" => theme::blue(),
                        "failed" => theme::red(),
                        _ => theme::text(),
                    }),
                ),
            ];

            if let Some(ref task_id) = r.task_id {
                lines.push(Line::styled(
                    format!("Task: {}", task_id),
                    Style::default().fg(theme::subtext0()),
                ));
            }

            let created = r.created_at.as_deref().unwrap_or("—");
            let updated = r.updated_at.as_deref().unwrap_or("—");
            lines.push(Line::styled(
                format!("Created: {}  Updated: {}", created, updated),
                Style::default().fg(theme::overlay0()),
            ));

            // Actions hint
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Actions: [n] New  [D] Delete",
                Style::default().fg(theme::overlay0()),
            ));

            // Prompt
            lines.push(Line::from(""));
            lines.push(Line::styled("Prompt:", Style::default().fg(theme::blue())));
            for line in r.prompt.lines() {
                lines.push(Line::styled(
                    format!("  {}", line),
                    Style::default().fg(theme::text()),
                ));
            }

            // Result
            if let Some(ref result) = r.result {
                lines.push(Line::from(""));
                lines.push(Line::styled("Result:", Style::default().fg(theme::green())));
                for line in result.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a report",
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
