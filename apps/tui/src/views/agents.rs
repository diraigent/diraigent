use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Agent table (left pane)
    let block = Block::default()
        .title(" Agents ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::blue()
        } else {
            theme::surface1()
        }));

    let header = Row::new(vec!["Name", "Status", "Capabilities", "Last Seen"])
        .style(Style::default().fg(theme::mauve()))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .agents
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let status = a.status.as_str();
            let status_color = match status {
                "idle" => theme::green(),
                "working" => theme::blue(),
                "offline" => theme::red(),
                _ => theme::text(),
            };
            let caps = if a.capabilities.is_empty() {
                "—".to_string()
            } else {
                a.capabilities.join(", ")
            };
            let last_seen = a
                .last_seen_at
                .as_deref()
                .and_then(|s| s.get(11..19))
                .unwrap_or("—");

            let style = if Some(i) == app.selected_agent {
                Style::default().fg(theme::base()).bg(theme::blue())
            } else {
                Style::default().fg(theme::text())
            };

            Row::new(vec![
                Cell::from(a.name.as_str()),
                Cell::from(status).style(Style::default().fg(status_color)),
                Cell::from(truncate(&caps, 30)),
                Cell::from(last_seen.to_string()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(15),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths).header(header).block(block);

    f.render_widget(table, chunks[0]);

    // Agent detail (right pane)
    let detail_block = Block::default()
        .title(" Agent Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::blue()
        } else {
            theme::surface1()
        }));

    let content = app
        .selected_agent
        .and_then(|i| app.agents.get(i))
        .map(|a| {
            let status_color = match a.status.as_str() {
                "idle" => theme::green(),
                "working" => theme::blue(),
                "offline" => theme::red(),
                _ => theme::text(),
            };
            let mut lines = vec![
                Line::styled(a.name.clone(), Style::default().fg(theme::blue())),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(theme::subtext0())),
                    Span::styled(a.status.clone(), Style::default().fg(status_color)),
                ]),
            ];

            // Owner ID
            if let Some(ref owner_id) = a.owner_id {
                lines.push(Line::from(vec![
                    Span::styled("Owner: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        owner_id.to_string()[..13].to_string(),
                        Style::default().fg(theme::text()),
                    ),
                ]));
            }

            // Capabilities as tags
            if !a.capabilities.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    "Capabilities:",
                    Style::default().fg(theme::peach()),
                ));
                lines.push(Line::styled(
                    format!("  {}", a.capabilities.join(", ")),
                    Style::default().fg(theme::green()),
                ));
            }

            // Last seen
            if let Some(ref last_seen) = a.last_seen_at {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("Last seen: ", Style::default().fg(theme::subtext0())),
                    Span::styled(last_seen.clone(), Style::default().fg(theme::text())),
                ]));
            }

            // Agent task queue
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "[a] View task queue",
                Style::default().fg(theme::overlay0()),
            ));

            if !app.agent_tasks.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Task Queue ({}):", app.agent_tasks.len()),
                    Style::default().fg(theme::peach()),
                ));
                for t in &app.agent_tasks {
                    let state_color = match t.state.as_str() {
                        "done" => theme::green(),
                        "cancelled" => theme::red(),
                        "ready" => theme::blue(),
                        _ => theme::yellow(),
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  #{} ", t.number),
                            Style::default().fg(theme::overlay1()),
                        ),
                        Span::styled(format!("[{}] ", t.state), Style::default().fg(state_color)),
                        Span::styled(t.title.clone(), Style::default().fg(theme::text())),
                    ]));
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select an agent",
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

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", truncated)
    } else {
        truncated
    }
}
