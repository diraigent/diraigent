use chrono::{DateTime, Local, Utc};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;
use crate::widgets::state_badge;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(6)])
        .split(chunks[0]);

    // Task list (stateful for scrolling)
    render_task_list(f, left_chunks[0], app);

    // Blockers panel
    render_blockers(f, left_chunks[1], app);

    // Detail panel
    render_detail(f, chunks[1], app);
}

fn render_task_list(f: &mut Frame, area: Rect, app: &mut App) {
    let title = if app.bulk_mode {
        format!(" Tasks [BULK: {} selected] ", app.bulk_selected.len())
    } else if app.show_hierarchy {
        if app.search_query.is_empty() {
            " Tasks [hierarchy] ".to_string()
        } else {
            format!(" Tasks [hierarchy|filter: {}] ", app.search_query)
        }
    } else if app.search_query.is_empty() {
        " Tasks ".to_string()
    } else {
        format!(" Tasks [filter: {}] ", app.search_query)
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.bulk_mode {
            theme::peach()
        } else if app.focus == 0 {
            theme::blue()
        } else {
            theme::surface1()
        }));

    let visible_indices = app.filtered_task_indices();
    let items: Vec<ListItem> = visible_indices
        .iter()
        .map(|&i| {
            let t = &app.tasks[i];
            let icon = state_badge::task_state_icon(&t.state);
            let style = state_badge::task_state_style(&t.state);

            let agent_tag = t
                .assigned_agent_id
                .and_then(|aid| {
                    app.agents.iter().find(|a| a.id == aid).map(|a| {
                        let short: String = a.name.chars().filter(|c| c.is_uppercase()).collect();
                        format!(" [{}]", short)
                    })
                })
                .unwrap_or_default();

            let timestamp = format_short_timestamp(t.created_at.as_deref());

            // Bulk selection marker
            let bulk_marker = if app.bulk_mode {
                if app.bulk_selected.contains(&t.id) {
                    "● "
                } else {
                    "○ "
                }
            } else {
                ""
            };

            // Flagged marker
            let flag_marker = if t.flagged { "⚑ " } else { "" };

            // Cost display
            let cost_str = if t.cost_usd > 0.0 {
                format!(" ${:.2}", t.cost_usd)
            } else {
                String::new()
            };

            // Hierarchy indicator for child tasks
            let hierarchy_prefix = if app.show_hierarchy && t.parent_id.is_some() {
                "  └─ "
            } else {
                ""
            };

            ListItem::new(Line::from(vec![
                Span::styled(bulk_marker.to_string(), Style::default().fg(theme::peach())),
                Span::styled(
                    flag_marker.to_string(),
                    Style::default().fg(theme::yellow()),
                ),
                Span::styled(
                    hierarchy_prefix.to_string(),
                    Style::default().fg(theme::overlay0()),
                ),
                Span::styled(
                    format!(" {} ", timestamp),
                    Style::default().fg(theme::overlay0()),
                ),
                Span::styled(format!("{} {:9} ", icon, t.state), style),
                Span::styled(
                    format!("#{} ", t.number),
                    Style::default().fg(theme::overlay1()),
                ),
                Span::styled(t.title.clone(), Style::default().fg(theme::text())),
                Span::styled(agent_tag, Style::default().fg(theme::mauve())),
                Span::styled(cost_str, Style::default().fg(theme::subtext0())),
            ]))
        })
        .collect();

    // Adjust list_state to match filtered position
    let filtered_pos = app
        .selected_task
        .and_then(|sel| visible_indices.iter().position(|&i| i == sel));
    app.task_list_state.select(filtered_pos);

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(theme::base()).bg(theme::blue()));
    f.render_stateful_widget(list, area, &mut app.task_list_state);
}

fn render_blockers(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Blockers ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::red()));

    let blockers: Vec<ListItem> = app
        .task_updates
        .iter()
        .filter(|u| u.kind == "blocker")
        .map(|u| {
            let time = u
                .created_at
                .as_deref()
                .and_then(|s| s.get(11..16))
                .unwrap_or("??:??");
            ListItem::new(Line::styled(
                format!(" {} {}", time, truncate(&u.content, 50)),
                Style::default().fg(theme::red()),
            ))
        })
        .collect();

    if blockers.is_empty() {
        let p = Paragraph::new(" (none)")
            .style(Style::default().fg(theme::overlay0()))
            .block(block);
        f.render_widget(p, area);
    } else {
        let list = List::new(blockers).block(block);
        f.render_widget(list, area);
    }
}

fn render_detail(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::blue()
        } else {
            theme::surface1()
        }));

    let task = app.selected_task.and_then(|i| app.tasks.get(i));

    let content = if let Some(t) = task {
        let agent_name = t
            .assigned_agent_id
            .and_then(|aid| {
                app.agents
                    .iter()
                    .find(|a| a.id == aid)
                    .map(|a| a.name.clone())
            })
            .unwrap_or_else(|| "unassigned".to_string());

        let created = format_short_timestamp(t.created_at.as_deref());

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("#{} ", t.number),
                    Style::default().fg(theme::overlay1()),
                ),
                Span::styled(t.title.clone(), Style::default().fg(theme::text())),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Kind: ", Style::default().fg(theme::subtext0())),
                Span::styled(
                    if t.kind.is_empty() { "—" } else { &t.kind },
                    Style::default().fg(theme::text()),
                ),
                Span::styled("  Priority: ", Style::default().fg(theme::subtext0())),
                Span::styled(t.priority.to_string(), Style::default().fg(theme::text())),
            ]),
            Line::from(vec![
                Span::styled("Agent: ", Style::default().fg(theme::subtext0())),
                Span::styled(agent_name.clone(), Style::default().fg(theme::mauve())),
            ]),
            Line::from(vec![
                Span::styled("Created: ", Style::default().fg(theme::subtext0())),
                Span::styled(created, Style::default().fg(theme::text())),
                if t.flagged {
                    Span::styled("  ⚑ Flagged", Style::default().fg(theme::yellow()))
                } else {
                    Span::styled("", Style::default())
                },
            ]),
            Line::from(vec![
                Span::styled("Cost: ", Style::default().fg(theme::subtext0())),
                Span::styled(
                    format!("${:.4}", t.cost_usd),
                    Style::default().fg(theme::text()),
                ),
            ]),
        ];

        // Parent task link
        if let Some(parent_id) = t.parent_id {
            let parent_info = app
                .tasks
                .iter()
                .find(|pt| pt.id == parent_id)
                .map(|pt| format!("#{} {}", pt.number, pt.title))
                .unwrap_or_else(|| format!("{}", parent_id));
            lines.push(Line::from(vec![
                Span::styled("Parent: ", Style::default().fg(theme::subtext0())),
                Span::styled(parent_info, Style::default().fg(theme::blue())),
            ]));
        }

        // Plan membership
        if let Some(plan_id) = t.plan_id {
            lines.push(Line::from(vec![
                Span::styled("Plan: ", Style::default().fg(theme::subtext0())),
                Span::styled(format!("{}", plan_id), Style::default().fg(theme::mauve())),
            ]));
        }

        // Subtasks section
        if !app.subtasks.is_empty() {
            lines.push(Line::styled(
                "── Subtasks ──",
                Style::default().fg(theme::surface1()),
            ));
            for sub in &app.subtasks {
                let sub_icon = state_badge::task_state_icon(&sub.state);
                let sub_style = state_badge::task_state_style(&sub.state);
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", sub_icon), sub_style),
                    Span::styled(
                        format!("#{} ", sub.number),
                        Style::default().fg(theme::overlay1()),
                    ),
                    Span::styled(format!("{:9} ", sub.state), sub_style),
                    Span::styled(sub.title.clone(), Style::default().fg(theme::text())),
                ]));
            }
            lines.push(Line::from(""));
        }

        // Git branch status (if available)
        if let Some(ref git_status) = app.git_task_status {
            if git_status.exists {
                let branch_name = git_status.branch.as_deref().unwrap_or("unknown");
                lines.push(Line::from(vec![
                    Span::styled("Branch: ", Style::default().fg(theme::subtext0())),
                    Span::styled(branch_name.to_string(), Style::default().fg(theme::green())),
                    Span::styled(
                        format!(
                            "  ↑{} ↓{}  {} files",
                            git_status.ahead, git_status.behind, git_status.changed_files_count
                        ),
                        Style::default().fg(theme::overlay0()),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));

        // Show task description/spec from context
        {
            let context = &t.context;
            if let Some(spec) = context.get("spec").and_then(|s| s.as_str()) {
                if !spec.is_empty() {
                    lines.push(Line::styled(
                        "── Description ──",
                        Style::default().fg(theme::surface1()),
                    ));
                    for line in spec.lines() {
                        lines.push(Line::styled(
                            line.to_string(),
                            Style::default().fg(theme::text()),
                        ));
                    }
                    lines.push(Line::from(""));
                }
            }

            if let Some(files) = context.get("files").and_then(|f| f.as_array()) {
                if !files.is_empty() {
                    lines.push(Line::styled(
                        "── Files ──",
                        Style::default().fg(theme::surface1()),
                    ));
                    for file in files {
                        if let Some(path) = file.as_str() {
                            lines.push(Line::styled(
                                format!("  {}", path),
                                Style::default().fg(theme::green()),
                            ));
                        }
                    }
                    lines.push(Line::from(""));
                }
            }

            if let Some(test_cmd) = context.get("test_cmd").and_then(|s| s.as_str()) {
                if !test_cmd.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Test: ", Style::default().fg(theme::subtext0())),
                        Span::styled(test_cmd.to_string(), Style::default().fg(theme::green())),
                    ]));
                    lines.push(Line::from(""));
                }
            }

            if let Some(acceptance) = context.get("acceptance").and_then(|a| a.as_array()) {
                if !acceptance.is_empty() {
                    lines.push(Line::styled(
                        "── Acceptance ──",
                        Style::default().fg(theme::surface1()),
                    ));
                    for criterion in acceptance {
                        if let Some(text) = criterion.as_str() {
                            lines.push(Line::styled(
                                format!("  • {}", text),
                                Style::default().fg(theme::text()),
                            ));
                        }
                    }
                    lines.push(Line::from(""));
                }
            }
        }

        // Changed files section (when toggled with F)
        if app.show_changed_files && !app.changed_files.is_empty() {
            lines.push(Line::styled(
                "── Changed Files ──",
                Style::default().fg(theme::surface1()),
            ));
            for cf in &app.changed_files {
                let status_icon = match cf.status.as_str() {
                    "added" | "A" => "+",
                    "deleted" | "D" => "-",
                    "modified" | "M" => "~",
                    "renamed" | "R" => "→",
                    _ => "?",
                };
                let color = match cf.status.as_str() {
                    "added" | "A" => theme::green(),
                    "deleted" | "D" => theme::red(),
                    _ => theme::yellow(),
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", status_icon), Style::default().fg(color)),
                    Span::styled(cf.path.clone(), Style::default().fg(theme::text())),
                    Span::styled(
                        format!("  +{} -{}", cf.additions, cf.deletions),
                        Style::default().fg(theme::overlay0()),
                    ),
                ]));
            }
            lines.push(Line::from(""));
        }

        // Dependencies section
        {
            let depends_on = &app.task_dependencies.depends_on;
            let blocks = &app.task_dependencies.blocks;

            if !depends_on.is_empty() || !blocks.is_empty() {
                lines.push(Line::styled(
                    "── Dependencies ──",
                    Style::default().fg(theme::surface1()),
                ));

                if !depends_on.is_empty() {
                    lines.push(Line::styled(
                        "Depends on:",
                        Style::default().fg(theme::peach()),
                    ));
                    for dep in depends_on {
                        let icon = state_badge::task_state_icon(&dep.state);
                        let title = if dep.title.is_empty() {
                            format!("  {}", dep.depends_on)
                        } else {
                            format!("  {} {} {}", icon, dep.state, dep.title)
                        };
                        lines.push(Line::styled(title, Style::default().fg(theme::text())));
                    }
                }

                if !blocks.is_empty() {
                    lines.push(Line::styled("Blocks:", Style::default().fg(theme::red())));
                    for dep in blocks {
                        let icon = state_badge::task_state_icon(&dep.state);
                        let title = if dep.title.is_empty() {
                            format!("  {}", dep.task_id)
                        } else {
                            format!("  {} {} {}", icon, dep.state, dep.title)
                        };
                        lines.push(Line::styled(title, Style::default().fg(theme::text())));
                    }
                }

                lines.push(Line::from(""));
            }
        }

        lines.push(Line::styled(
            "── Updates ──",
            Style::default().fg(theme::surface1()),
        ));

        for u in &app.task_updates {
            let time = u
                .created_at
                .as_deref()
                .and_then(|s| s.get(11..16))
                .unwrap_or("??:??");
            let color = state_badge::update_kind_color(&u.kind);
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", time), Style::default().fg(theme::overlay0())),
                Span::styled(format!("[{}] ", u.kind), Style::default().fg(color)),
                Span::styled(u.content.clone(), Style::default().fg(theme::text())),
            ]));
        }

        if app.task_updates.is_empty() {
            lines.push(Line::styled(
                " No updates yet",
                Style::default().fg(theme::overlay0()),
            ));
        }

        // Comments section
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "── Comments ──",
            Style::default().fg(theme::surface1()),
        ));

        for c in &app.task_comments {
            let time = c
                .created_at
                .as_deref()
                .and_then(|s| s.get(11..16))
                .unwrap_or("??:??");
            let author = c.author_name.as_deref().unwrap_or("human");
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", time), Style::default().fg(theme::overlay0())),
                Span::styled(
                    format!("[{}] ", author),
                    Style::default().fg(theme::mauve()),
                ),
                Span::styled(c.content.clone(), Style::default().fg(theme::text())),
            ]));
        }

        if app.task_comments.is_empty() {
            lines.push(Line::styled(
                " No comments yet",
                Style::default().fg(theme::overlay0()),
            ));
        }

        lines
    } else {
        vec![Line::styled(
            " Select a task to view details",
            Style::default().fg(theme::overlay0()),
        )]
    };

    // Clamp scroll to content length
    let content_len = content.len() as u16;
    let inner_height = area.height.saturating_sub(2);
    if content_len <= inner_height {
        app.detail_scroll = 0;
    } else if app.detail_scroll > content_len.saturating_sub(inner_height) {
        app.detail_scroll = content_len.saturating_sub(inner_height);
    }

    let p = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.detail_scroll, 0))
        .style(Style::default().bg(theme::base()));
    f.render_widget(p, area);
}

fn format_short_timestamp(iso: Option<&str>) -> String {
    let Some(s) = iso else {
        return "—".into();
    };
    let Ok(utc) = s.parse::<DateTime<Utc>>() else {
        return s.get(..16).unwrap_or(s).to_string();
    };
    let local = utc.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M").to_string()
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
