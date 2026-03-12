use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Cell, Row, Table};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Agents ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::blue()));

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
        ratatui::layout::Constraint::Percentage(25),
        ratatui::layout::Constraint::Percentage(15),
        ratatui::layout::Constraint::Percentage(35),
        ratatui::layout::Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths).header(header).block(block);

    f.render_widget(table, area);
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
