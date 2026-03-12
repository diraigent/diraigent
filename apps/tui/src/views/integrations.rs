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
        .title(" Integrations ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::teal()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .integrations
        .iter()
        .enumerate()
        .map(|(i, intg)| {
            let kind = intg.kind.as_deref().unwrap_or("?");
            let provider = intg.provider.as_deref().unwrap_or("");
            let enabled = intg.enabled;
            let label = if !provider.is_empty() {
                format!(" [{}] [{}] {}", kind, provider, intg.name)
            } else {
                format!(" [{}] {}", kind, intg.name)
            };
            let color = if enabled {
                theme::green()
            } else {
                theme::red()
            };
            let style = if Some(i) == app.selected_integration {
                Style::default().fg(theme::base()).bg(theme::teal())
            } else {
                Style::default().fg(color)
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), chunks[0]);

    // Detail
    let detail_block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::teal()
        } else {
            theme::surface1()
        }));

    let content = app
        .selected_integration
        .and_then(|i| app.integrations.get(i))
        .map(|intg| {
            let enabled = intg.enabled;
            let mut lines = vec![
                Line::styled(&intg.name, Style::default().fg(theme::teal())),
                Line::from(""),
                Line::styled(
                    format!("Kind: {}", intg.kind.as_deref().unwrap_or("—")),
                    Style::default().fg(theme::text()),
                ),
                Line::styled(
                    format!("Provider: {}", intg.provider.as_deref().unwrap_or("—")),
                    Style::default().fg(theme::text()),
                ),
                Line::styled(
                    format!("Base URL: {}", intg.base_url.as_deref().unwrap_or("—")),
                    Style::default().fg(theme::text()),
                ),
                Line::styled(
                    format!("Auth type: {}", intg.auth_type.as_deref().unwrap_or("—")),
                    Style::default().fg(theme::text()),
                ),
                Line::styled(
                    format!("Enabled: {}", if enabled { "yes" } else { "no" }),
                    Style::default().fg(if enabled {
                        theme::green()
                    } else {
                        theme::red()
                    }),
                ),
            ];

            if let Some(caps) = &intg.capabilities {
                if !caps.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::styled(
                        "Capabilities:",
                        Style::default().fg(theme::blue()),
                    ));
                    for cap in caps {
                        lines.push(Line::styled(
                            format!("  • {}", cap),
                            Style::default().fg(theme::text()),
                        ));
                    }
                }
            }

            if let Some(config) = &intg.config {
                lines.push(Line::from(""));
                lines.push(Line::styled("Config:", Style::default().fg(theme::blue())));
                let pretty = serde_json::to_string_pretty(config).unwrap_or_default();
                for line in pretty.lines() {
                    lines.push(Line::styled(
                        format!("  {}", line),
                        Style::default().fg(theme::subtext0()),
                    ));
                }
            }

            // Show agent access list
            if !app.integration_access.is_empty() {
                let access_for_this: Vec<_> = app
                    .integration_access
                    .iter()
                    .filter(|a| a.integration_id == intg.id)
                    .collect();
                if !access_for_this.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::styled(
                        "Agent Access:",
                        Style::default().fg(theme::blue()),
                    ));
                    for access in &access_for_this {
                        let agent_name = app
                            .agents
                            .iter()
                            .find(|a| a.id == access.agent_id)
                            .map(|a| a.name.as_str())
                            .unwrap_or("unknown");
                        lines.push(Line::styled(
                            format!("  • {}", agent_name),
                            Style::default().fg(theme::text()),
                        ));
                    }
                }
            }

            // Actions hint
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Actions: [e] Toggle  [a] Access  [D] Delete",
                Style::default().fg(theme::overlay0()),
            ));

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select an integration",
                Style::default().fg(theme::overlay0()),
            )]
        });

    // Clamp scroll
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
