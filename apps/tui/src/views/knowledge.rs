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
        .title(" Knowledge ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::teal()));

    let items: Vec<ListItem> = app
        .knowledge
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let cat = k.category.as_deref().unwrap_or("—");
            let style = if Some(i) == app.selected_knowledge {
                Style::default().fg(theme::base()).bg(theme::teal())
            } else {
                Style::default().fg(theme::text())
            };
            ListItem::new(Line::styled(format!(" [{}] {}", cat, k.title), style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Content ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_knowledge
        .and_then(|i| app.knowledge.get(i))
        .map(|k| {
            let tags = k.tags.as_ref().map(|t| t.join(", ")).unwrap_or_default();
            let mut lines = vec![
                Line::styled(&k.title, Style::default().fg(theme::teal())),
                Line::styled(
                    format!("Tags: {}", tags),
                    Style::default().fg(theme::subtext0()),
                ),
                Line::from(""),
            ];
            if let Some(c) = &k.content {
                lines.push(Line::styled(c.as_str(), Style::default().fg(theme::text())));
            }
            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select an entry",
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
