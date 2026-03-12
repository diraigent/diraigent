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
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    render_roles(f, chunks[0], app);
    render_members(f, chunks[1], app);
    render_detail(f, chunks[2], app);
}

fn render_roles(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Roles ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.team_focus == 0 {
            theme::mauve()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .roles
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let auth_count = r.authorities.len();
            let label = if auth_count > 0 {
                format!(" {} ({} auth)", r.name, auth_count)
            } else {
                format!(" {}", r.name)
            };
            let style = if Some(i) == app.selected_role {
                Style::default().fg(theme::base()).bg(theme::mauve())
            } else {
                Style::default().fg(theme::text())
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), area);
}

fn render_members(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Members ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.team_focus == 1 {
            theme::mauve()
        } else {
            theme::surface1()
        }));

    // Filter members by selected role
    let filtered: Vec<(usize, &crate::client::Member)> = app
        .members
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            app.selected_role
                .and_then(|i| app.roles.get(i))
                .map(|r| m.role_id == Some(r.id))
                .unwrap_or(true) // show all if no role selected
        })
        .collect();

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|(i, m)| {
            let agent_name = m
                .agent_id
                .and_then(|aid| app.agents.iter().find(|a| a.id == aid))
                .map(|a| a.name.as_str())
                .unwrap_or("unknown");
            let status = m.status.as_deref().unwrap_or("unknown");
            let status_color = match status {
                "active" => theme::green(),
                "inactive" => theme::yellow(),
                "suspended" => theme::red(),
                _ => theme::text(),
            };
            let style = if Some(*i) == app.selected_member {
                Style::default().fg(theme::base()).bg(theme::mauve())
            } else {
                Style::default().fg(theme::text())
            };
            let status_style = if Some(*i) == app.selected_member {
                style
            } else {
                Style::default().fg(status_color)
            };
            // Simple line with agent name and status
            let label = format!(" {} [{}]", agent_name, status);
            let _ = status_style; // use status_color for unselected items
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    f.render_widget(List::new(items).block(block), area);
}

fn render_detail(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.team_focus == 2 {
            theme::mauve()
        } else {
            theme::surface1()
        }));

    let content = if app.team_focus == 0 {
        // Show selected role detail
        app.selected_role
            .and_then(|i| app.roles.get(i))
            .map(|r| {
                let mut lines = vec![
                    Line::styled(&r.name, Style::default().fg(theme::mauve())),
                    Line::from(""),
                ];

                if let Some(desc) = &r.description {
                    lines.push(Line::styled(
                        format!("Description: {}", desc),
                        Style::default().fg(theme::text()),
                    ));
                    lines.push(Line::from(""));
                }

                if !r.authorities.is_empty() {
                    lines.push(Line::styled(
                        "Authorities:",
                        Style::default().fg(theme::blue()),
                    ));
                    for auth in &r.authorities {
                        lines.push(Line::styled(
                            format!("  • {}", auth),
                            Style::default().fg(theme::text()),
                        ));
                    }
                    lines.push(Line::from(""));
                }

                if !r.required_capabilities.is_empty() {
                    lines.push(Line::styled(
                        format!("Capabilities: {}", r.required_capabilities.join(", ")),
                        Style::default().fg(theme::subtext0()),
                    ));
                    lines.push(Line::from(""));
                }

                if !r.knowledge_scope.is_empty() {
                    lines.push(Line::styled(
                        format!("Knowledge scope: {}", r.knowledge_scope.join(", ")),
                        Style::default().fg(theme::subtext0()),
                    ));
                }

                lines
            })
            .unwrap_or_else(|| {
                vec![Line::styled(
                    " Select a role",
                    Style::default().fg(theme::overlay0()),
                )]
            })
    } else {
        // Show selected member detail
        app.selected_member
            .and_then(|i| app.members.get(i))
            .map(|m| {
                let agent_name = m
                    .agent_id
                    .and_then(|aid| app.agents.iter().find(|a| a.id == aid))
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());

                let role_name = m
                    .role_id
                    .and_then(|rid| app.roles.iter().find(|r| r.id == rid))
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| "—".to_string());

                let status = m.status.as_deref().unwrap_or("unknown");
                let status_color = match status {
                    "active" => theme::green(),
                    "inactive" => theme::yellow(),
                    "suspended" => theme::red(),
                    _ => theme::text(),
                };

                let mut lines = vec![
                    Line::styled(agent_name, Style::default().fg(theme::mauve())),
                    Line::from(""),
                    Line::styled(
                        format!("Role: {}", role_name),
                        Style::default().fg(theme::text()),
                    ),
                    Line::styled(
                        format!("Status: {}", status),
                        Style::default().fg(status_color),
                    ),
                ];

                if let Some(joined) = &m.joined_at {
                    lines.push(Line::styled(
                        format!("Joined: {}", joined.get(..16).unwrap_or(joined)),
                        Style::default().fg(theme::subtext0()),
                    ));
                }

                if let Some(config) = &m.config {
                    lines.push(Line::from(""));
                    lines.push(Line::styled("Config:", Style::default().fg(theme::blue())));
                    lines.push(Line::styled(
                        serde_json::to_string_pretty(config).unwrap_or_default(),
                        Style::default().fg(theme::subtext0()),
                    ));
                }

                lines
            })
            .unwrap_or_else(|| {
                vec![Line::styled(
                    " Select a member",
                    Style::default().fg(theme::overlay0()),
                )]
            })
    };

    // Clamp scroll
    let content_len = content.len() as u16;
    let inner_height = area.height.saturating_sub(2);
    if content_len <= inner_height {
        app.detail_scroll = 0;
    } else if app.detail_scroll > content_len.saturating_sub(inner_height) {
        app.detail_scroll = content_len.saturating_sub(inner_height);
    }

    f.render_widget(
        Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((app.detail_scroll, 0)),
        area,
    );
}
