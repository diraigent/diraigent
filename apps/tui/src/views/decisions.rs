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

    let block = Block::default()
        .title(" Decisions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::mauve()));

    let items: Vec<ListItem> = app
        .decisions
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let status = d.status.as_deref().unwrap_or("—");
            let color = match status {
                "accepted" => theme::green(),
                "rejected" => theme::red(),
                "proposed" => theme::yellow(),
                "superseded" => theme::overlay0(),
                "deprecated" => theme::surface1(),
                _ => theme::text(),
            };
            let style = if Some(i) == app.selected_decision {
                Style::default().fg(theme::base()).bg(theme::mauve())
            } else {
                Style::default().fg(color)
            };
            ListItem::new(Line::styled(format!(" [{}] {}", status, d.title), style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    let detail_block = Block::default()
        .title(" Decision Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let content = app
        .selected_decision
        .and_then(|i| app.decisions.get(i))
        .map(|d| {
            let mut lines = vec![
                Line::styled(&d.title, Style::default().fg(theme::mauve())),
                Line::styled(
                    format!("Status: {}", d.status.as_deref().unwrap_or("—")),
                    Style::default().fg(theme::subtext0()),
                ),
                Line::from(""),
            ];
            // Show available actions hint based on status
            let status = d.status.as_deref().unwrap_or("proposed");
            let actions = match status {
                "proposed" => "[a] Accept  [x] Reject  [D] Delete",
                "accepted" => "[S] Supersede  [X] Deprecate  [D] Delete",
                _ => "[D] Delete",
            };
            lines.push(Line::styled(
                format!("Actions: {}", actions),
                Style::default().fg(theme::overlay0()),
            ));

            if let Some(ctx) = &d.context {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Context: {}", ctx),
                    Style::default().fg(theme::text()),
                ));
            }
            if let Some(dec_text) = &d.decision {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Decision: {}", dec_text),
                    Style::default().fg(theme::text()),
                ));
            }
            if let Some(rationale) = &d.rationale {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Rationale: {}", rationale),
                    Style::default().fg(theme::subtext0()),
                ));
            }
            if let Some(alts) = &d.alternatives {
                if !alts.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::styled(
                        "Alternatives:",
                        Style::default().fg(theme::blue()),
                    ));
                    for alt in alts {
                        lines.push(Line::styled(
                            format!("  • {}", alt.name),
                            Style::default().fg(theme::subtext0()),
                        ));
                        if let Some(pros) = &alt.pros {
                            lines.push(Line::styled(
                                format!("    + {}", pros),
                                Style::default().fg(theme::green()),
                            ));
                        }
                        if let Some(cons) = &alt.cons {
                            lines.push(Line::styled(
                                format!("    - {}", cons),
                                Style::default().fg(theme::red()),
                            ));
                        }
                    }
                }
            }
            if let Some(consequences) = &d.consequences {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Consequences: {}", consequences),
                    Style::default().fg(theme::subtext0()),
                ));
            }
            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a decision",
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
