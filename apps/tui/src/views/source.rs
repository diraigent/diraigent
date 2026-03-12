use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Left: file tree
    let path_display = if app.source_current_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", app.source_current_path)
    };
    let block = Block::default()
        .title(format!(" Source: {} ", path_display))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let mut items: Vec<ListItem> = Vec::new();
    // Show ".." to go up if we're in a subdirectory
    if !app.source_current_path.is_empty() {
        let style = if app.source_selected == Some(0) {
            Style::default().fg(theme::base()).bg(theme::yellow())
        } else {
            Style::default().fg(theme::overlay0())
        };
        items.push(ListItem::new(Line::styled("  .. (up)", style)));
    }

    let offset = if app.source_current_path.is_empty() {
        0
    } else {
        1
    };
    for (i, entry) in app.source_entries.iter().enumerate() {
        let display_idx = i + offset;
        let (icon, color) = if entry.kind == "dir" {
            ("/", theme::blue())
        } else {
            ("", theme::text())
        };
        let label = format!("  {}{}", entry.name, icon);
        let style = if Some(display_idx) == app.source_selected {
            Style::default().fg(theme::base()).bg(theme::yellow())
        } else {
            Style::default().fg(color)
        };
        items.push(ListItem::new(Line::styled(label, style)));
    }

    let list = List::new(items).block(block);
    f.render_widget(list, chunks[0]);

    // Right: file content
    let blob_title = app
        .source_blob_path
        .as_deref()
        .map(|p| format!(" {} ", p))
        .unwrap_or_else(|| " File Content ".to_string());
    let detail_block = Block::default()
        .title(blob_title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let content_lines: Vec<Line> = if let Some(ref content) = app.source_blob_content {
        content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:>4} ", i + 1),
                        Style::default().fg(theme::overlay0()),
                    ),
                    Span::styled(line, Style::default().fg(theme::text())),
                ])
            })
            .collect()
    } else {
        vec![Line::styled(
            "  Select a file to view its contents",
            Style::default().fg(theme::overlay0()),
        )]
    };

    let content = Paragraph::new(content_lines)
        .block(detail_block)
        .scroll((app.source_blob_scroll, 0));
    f.render_widget(content, chunks[1]);
}
