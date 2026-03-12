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

    // Build filter label
    let kind_filter = app.verification_kind_filter.as_deref().unwrap_or("all");
    let status_filter = app.verification_status_filter.as_deref().unwrap_or("all");
    let list_title = format!(
        " Verifications [kind:{}  status:{}] ",
        kind_filter, status_filter
    );

    let block = Block::default()
        .title(list_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::mauve()));

    let filtered = app.filtered_verifications();
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let status_icon = match v.status.as_str() {
                "pass" => "✓",
                "fail" => "✗",
                "pending" => "○",
                "skipped" => "–",
                _ => "?",
            };
            let status_color = match v.status.as_str() {
                "pass" => theme::green(),
                "fail" => theme::red(),
                "pending" => theme::yellow(),
                "skipped" => theme::overlay0(),
                _ => theme::text(),
            };
            let style = if Some(i) == app.selected_verification {
                Style::default().fg(theme::base()).bg(theme::mauve())
            } else {
                Style::default().fg(status_color)
            };
            ListItem::new(Line::styled(
                format!(" {} [{}] {}", status_icon, v.kind, v.title),
                style,
            ))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail panel
    let detail_block = Block::default()
        .title(" Verification Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_verification
        .and_then(|i| {
            let filtered = app.filtered_verifications();
            filtered.get(i).cloned().cloned()
        })
        .map(|v| {
            let status_color = match v.status.as_str() {
                "pass" => theme::green(),
                "fail" => theme::red(),
                "pending" => theme::yellow(),
                "skipped" => theme::overlay0(),
                _ => theme::text(),
            };

            let mut lines = vec![
                Line::styled(v.title.clone(), Style::default().fg(theme::mauve())),
                Line::styled(
                    format!("Kind: {}  Status: {}", v.kind, v.status),
                    Style::default().fg(status_color),
                ),
            ];

            // Show linked task
            if let Some(tid) = v.task_id {
                let task_title = app
                    .tasks
                    .iter()
                    .find(|t| t.id == tid)
                    .map(|t| t.title.as_str())
                    .unwrap_or("(unknown)");
                lines.push(Line::styled(
                    format!("Task: {} ({})", task_title, &tid.to_string()[..8]),
                    Style::default().fg(theme::blue()),
                ));
            }

            if let Some(ref created) = v.created_at {
                lines.push(Line::styled(
                    format!("Created: {}", created),
                    Style::default().fg(theme::subtext0()),
                ));
            }

            // Detail text
            if let Some(ref detail) = v.detail {
                lines.push(Line::from(""));
                lines.push(Line::styled("Detail:", Style::default().fg(theme::teal())));
                for line in detail.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            // Evidence JSON
            if let Some(ref evidence) = v.evidence {
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

            // Actions hint
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Actions: [n] New  [s] Status  [K] Kind filter  [S] Status filter",
                Style::default().fg(theme::overlay0()),
            ));

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a verification",
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
