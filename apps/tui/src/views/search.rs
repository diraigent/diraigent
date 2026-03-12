use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

fn entity_color(entity_type: &str) -> ratatui::style::Color {
    match entity_type {
        "task" => theme::blue(),
        "goal" => theme::green(),
        "decision" => theme::mauve(),
        "observation" => theme::peach(),
        "knowledge" => theme::teal(),
        "agent" => theme::yellow(),
        _ => theme::text(),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Left: results list
    let title = if app.search_executed_query.is_empty() {
        " Search — press / to search ".to_string()
    } else {
        format!(
            " Search: \"{}\" ({} results) ",
            app.search_executed_query, app.search_total
        )
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let color = entity_color(&r.entity_type);
            let label = format!(" [{}] {}", r.entity_type, r.title);
            let style = if Some(i) == app.selected_search_result {
                Style::default().fg(theme::base()).bg(theme::yellow())
            } else {
                Style::default().fg(color)
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, chunks[0]);

    // Right: detail
    let detail_block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let detail_text = if let Some(idx) = app.selected_search_result {
        if let Some(r) = app.search_results.get(idx) {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Type: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        &r.entity_type,
                        Style::default().fg(entity_color(&r.entity_type)),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Title: ", Style::default().fg(theme::subtext0())),
                    Span::styled(&r.title, Style::default().fg(theme::blue())),
                ]),
                Line::from(vec![
                    Span::styled("Relevance: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        format!("{:.1}%", r.relevance * 100.0),
                        Style::default().fg(theme::text()),
                    ),
                ]),
            ];
            if let Some(ref snippet) = r.snippet {
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Snippet:",
                    Style::default().fg(theme::subtext0()),
                ));
                for line in snippet.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::text()),
                    ));
                }
            }
            if let Some(ref created) = r.created_at {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled("Created: ", Style::default().fg(theme::subtext0())),
                    Span::styled(created, Style::default().fg(theme::overlay0())),
                ]));
            }
            lines
        } else {
            vec![Line::styled(
                "No result selected",
                Style::default().fg(theme::overlay0()),
            )]
        }
    } else {
        vec![Line::styled(
            "Press / to search across all entities",
            Style::default().fg(theme::overlay0()),
        )]
    };

    let detail = Paragraph::new(detail_text)
        .block(detail_block)
        .wrap(Wrap { trim: true });
    f.render_widget(detail, chunks[1]);
}
