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
        .title(" Playbooks ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::peach()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .playbooks
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let tags = p.tags.join(", ");
            let label = if tags.is_empty() {
                format!(" {}", p.title)
            } else {
                format!(" {} [{}]", p.title, tags)
            };
            let style = if Some(i) == app.selected_playbook {
                Style::default().fg(theme::base()).bg(theme::peach())
            } else {
                Style::default().fg(theme::text())
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Steps ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::peach()
        } else {
            theme::surface1()
        }));

    let content = app
        .selected_playbook
        .and_then(|i| app.playbooks.get(i))
        .map(|p| {
            let mut lines = vec![
                Line::styled(&p.title, Style::default().fg(theme::peach())),
                Line::from(""),
            ];

            if let Some(trigger) = &p.trigger_description {
                lines.push(Line::styled(
                    format!("Trigger: {}", trigger),
                    Style::default().fg(theme::subtext0()),
                ));
                lines.push(Line::from(""));
            }

            if !p.tags.is_empty() {
                lines.push(Line::styled(
                    format!("Tags: {}", p.tags.join(", ")),
                    Style::default().fg(theme::subtext0()),
                ));
                lines.push(Line::from(""));
            }

            // Render steps
            {
                let steps = &p.steps;
                lines.push(Line::styled("Steps:", Style::default().fg(theme::blue())));
                lines.push(Line::from(""));

                if let Some(arr) = steps.as_array() {
                    for step in arr {
                        let order = step
                            .get("step")
                            .or_else(|| step.get("order"))
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);

                        let name = step.get("name").and_then(|v| v.as_str()).unwrap_or("");

                        let desc = step
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let actor = step.get("actor").and_then(|v| v.as_str()).unwrap_or("");

                        let output = step.get("output").and_then(|v| v.as_str()).unwrap_or("");

                        let model = step.get("model").and_then(|v| v.as_str()).unwrap_or("");
                        let on_complete = step
                            .get("on_complete")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let timeout = step
                            .get("timeout_minutes")
                            .and_then(|v| v.as_i64())
                            .map(|v| format!(" ({}m)", v))
                            .unwrap_or_default();
                        let model_display = if model.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", model)
                        };

                        // Step header
                        let header = if !name.is_empty() {
                            if actor.is_empty() {
                                format!(
                                    "  {}. {}{}{} → {}",
                                    order, name, timeout, model_display, on_complete
                                )
                            } else {
                                format!(
                                    "  {}. {} ({}){}{} → {}",
                                    order, name, actor, timeout, model_display, on_complete
                                )
                            }
                        } else {
                            format!("  {}.", order)
                        };
                        lines.push(Line::styled(header, Style::default().fg(theme::green())));

                        // Description
                        if !desc.is_empty() {
                            lines.push(Line::styled(
                                format!("     {}", desc),
                                Style::default().fg(theme::text()),
                            ));
                        }

                        // Output
                        if !output.is_empty() {
                            lines.push(Line::styled(
                                format!("     -> {}", output),
                                Style::default().fg(theme::overlay0()),
                            ));
                        }

                        // Command if present
                        if let Some(cmd) = step.get("command").and_then(|v| v.as_str()) {
                            lines.push(Line::styled(
                                format!("     $ {}", cmd),
                                Style::default().fg(theme::yellow()),
                            ));
                        }

                        lines.push(Line::from(""));
                    }
                }
            }

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a playbook",
                Style::default().fg(theme::overlay0()),
            )]
        });

    // Clamp scroll to content length
    let content_len = content.len() as u16;
    let inner_height = chunks[1].height.saturating_sub(2); // minus borders
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
