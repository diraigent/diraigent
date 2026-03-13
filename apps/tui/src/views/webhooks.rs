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

    // ── Left panel: webhook list ────────────────────────────────
    let block = Block::default()
        .title(" Webhooks ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::teal()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .webhooks
        .iter()
        .enumerate()
        .map(|(i, wh)| {
            let url_display = if wh.url.len() > 35 {
                format!("{}…", &wh.url[..34])
            } else {
                wh.url.clone()
            };
            let events_display = if wh.events.is_empty() {
                "all".to_string()
            } else if wh.events.len() <= 2 {
                wh.events.join(", ")
            } else {
                format!("{}, +{}", wh.events[0], wh.events.len() - 1)
            };
            let enabled_icon = if wh.enabled { "✓" } else { "✗" };
            let label = format!(" {} {} [{}]", enabled_icon, url_display, events_display);
            let color = if wh.enabled {
                theme::green()
            } else {
                theme::red()
            };
            let style = if Some(i) == app.selected_webhook {
                Style::default().fg(theme::base()).bg(theme::teal())
            } else {
                Style::default().fg(color)
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    if items.is_empty() {
        let empty = Paragraph::new(" No webhooks. Press [n] to create one.")
            .block(block)
            .style(Style::default().fg(theme::overlay0()));
        f.render_widget(empty, chunks[0]);
    } else {
        f.render_widget(List::new(items).block(block), chunks[0]);
    }

    // ── Right panel: detail + deliveries ────────────────────────
    let detail_block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::teal()
        } else {
            theme::surface1()
        }));

    let content = app
        .selected_webhook
        .and_then(|i| app.webhooks.get(i))
        .map(|wh| {
            let mut lines = vec![
                Line::styled(wh.name.clone(), Style::default().fg(theme::teal())),
                Line::from(""),
                Line::styled(
                    format!("URL: {}", wh.url),
                    Style::default().fg(theme::text()),
                ),
                Line::styled(
                    format!("Enabled: {}", if wh.enabled { "yes" } else { "no" }),
                    Style::default().fg(if wh.enabled {
                        theme::green()
                    } else {
                        theme::red()
                    }),
                ),
            ];

            // Secret (masked)
            if let Some(ref secret) = wh.secret {
                let masked = if secret.len() > 4 {
                    format!("{}****", &secret[..4])
                } else {
                    "****".to_string()
                };
                lines.push(Line::styled(
                    format!("Secret: {}", masked),
                    Style::default().fg(theme::text()),
                ));
            }

            // Created at
            if let Some(ref created) = wh.created_at {
                lines.push(Line::styled(
                    format!("Created: {}", &created[..19.min(created.len())]),
                    Style::default().fg(theme::subtext0()),
                ));
            }

            // Events
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Subscribed Events:",
                Style::default().fg(theme::blue()),
            ));
            if wh.events.is_empty() {
                lines.push(Line::styled(
                    "  (all events)",
                    Style::default().fg(theme::subtext0()),
                ));
            } else {
                for ev in &wh.events {
                    lines.push(Line::styled(
                        format!("  • {}", ev),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            // Test result
            if let Some(ref result) = app.webhook_test_result {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    format!("Last test: {}", result),
                    Style::default().fg(theme::yellow()),
                ));
            }

            // Deliveries
            if !app.webhook_deliveries.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(
                    "Recent Deliveries:",
                    Style::default().fg(theme::blue()),
                ));
                for del in app.webhook_deliveries.iter().take(20) {
                    let status_str = del
                        .response_status
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "—".to_string());
                    let success_icon = if del.success { "✓" } else { "✗" };
                    let ts = del
                        .delivered_at
                        .as_deref()
                        .map(|t| &t[..19.min(t.len())])
                        .unwrap_or("—");
                    let color = if del.success {
                        theme::green()
                    } else {
                        theme::red()
                    };
                    lines.push(Line::styled(
                        format!(
                            "  {} [{}] {} — {}",
                            success_icon, status_str, del.event_type, ts
                        ),
                        Style::default().fg(color),
                    ));
                }
            }

            // Actions hint
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Actions: [n]ew  [e] Toggle  [t] Test  [D] Delete  [Enter] Deliveries",
                Style::default().fg(theme::overlay0()),
            ));

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a webhook",
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
