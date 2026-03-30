use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::client::Work;
use crate::theme;
use crate::widgets::state_badge;

/// Map API work status to user-friendly display name.
pub fn work_status_label(status: &str) -> &str {
    match status {
        "achieved" => "Done",
        "paused" => "Pause",
        "abandoned" => "Abandon",
        other => other,
    }
}

fn make_work_item(
    app: &App,
    g: &Work,
    i: usize,
    selected: Option<usize>,
    is_focused_section: bool,
) -> ListItem<'static> {
    let status = g.status.as_deref().unwrap_or("active");
    let work_type = g.work_type.as_deref().unwrap_or("epic");
    let color = match status {
        "active" => theme::green(),
        "achieved" => theme::blue(),
        "paused" => theme::yellow(),
        "abandoned" => theme::red(),
        _ => theme::text(),
    };
    let style = if is_focused_section && Some(i) == selected {
        Style::default().fg(theme::base()).bg(theme::green())
    } else {
        Style::default().fg(color)
    };
    let progress_str = app
        .work_progress_map
        .get(&g.id)
        .map(|p| format!(" ({}/{})", p.done_tasks, p.total_tasks))
        .unwrap_or_default();
    ListItem::new(Line::styled(
        format!(
            " [{}:{}] {}{}",
            work_type,
            work_status_label(status),
            g.title,
            progress_str
        ),
        style,
    ))
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    // Split into top (work list + info) and bottom (task detail)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_top(f, main_chunks[0], app);
    render_task_detail(f, main_chunks[1], app);
}

/// Top half: work list (left) + work info & task list (right)
fn render_top(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Split left panel into active (top) and done (bottom) sections
    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[0]);

    let work_list_focused = app.work_focus == 0;
    let active_focused = work_list_focused && app.work_section == 0;
    let done_focused = work_list_focused && app.work_section == 1;

    // Active/Paused list (top)
    {
        let border_color = if active_focused {
            theme::green()
        } else {
            theme::surface1()
        };
        let block = Block::default()
            .title(" Active ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let items: Vec<ListItem> = app
            .work_items
            .iter()
            .enumerate()
            .map(|(i, g)| make_work_item(app, g, i, app.selected_work, active_focused))
            .collect();

        app.work_list_state.select(app.selected_work);
        f.render_stateful_widget(
            List::new(items).block(block),
            list_chunks[0],
            &mut app.work_list_state,
        );
    }

    // Done/Abandoned list (bottom)
    {
        let border_color = if done_focused {
            theme::green()
        } else {
            theme::surface1()
        };
        let title = if app.done_work_has_more || app.done_work_page > 0 {
            format!(" Done (p{}) ", app.done_work_page + 1)
        } else {
            " Done ".to_string()
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let items: Vec<ListItem> = app
            .done_work_items
            .iter()
            .enumerate()
            .map(|(i, g)| make_work_item(app, g, i, app.selected_done_work, done_focused))
            .collect();

        app.done_work_list_state.select(app.selected_done_work);
        f.render_stateful_widget(
            List::new(items).block(block),
            list_chunks[1],
            &mut app.done_work_list_state,
        );
    }

    // Right panel: work info (top) + task list (bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    render_work_info(f, right_chunks[0], app);
    render_task_list(f, right_chunks[1], app);
}

/// Condensed work info panel (top-right)
fn render_work_info(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .title(" Work Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let selected_goal = if app.work_section == 0 {
        app.selected_work.and_then(|i| app.work_items.get(i))
    } else {
        app.selected_done_work
            .and_then(|i| app.done_work_items.get(i))
    };

    let content = selected_goal
        .map(|g| {
            let status = g.status.as_deref().unwrap_or("active");
            let work_type = g.work_type.as_deref().unwrap_or("epic");
            let auto_status = g.auto_status.unwrap_or(false);
            let status_color = match status {
                "active" => theme::green(),
                "achieved" => theme::blue(),
                "paused" => theme::yellow(),
                "abandoned" => theme::red(),
                _ => theme::text(),
            };
            let mut lines = vec![
                Line::styled(g.title.clone(), Style::default().fg(theme::green())),
                Line::styled(
                    format!("Type: {}  Status: {}", work_type, work_status_label(status),),
                    Style::default().fg(status_color),
                ),
            ];

            if auto_status {
                lines.push(Line::styled(
                    "Auto-status: ON",
                    Style::default().fg(theme::peach()),
                ));
            }

            if let Some(ref desc) = g.description {
                if !desc.is_empty() {
                    lines.push(Line::styled(
                        desc.as_str(),
                        Style::default().fg(theme::text()),
                    ));
                }
            }

            // Progress bar
            if let Some(ref progress) = app.work_progress {
                if progress.work_id == g.id {
                    let total = progress.total_tasks;
                    let done = progress.done_tasks;
                    let pct = if total > 0 {
                        (done as f64 / total as f64 * 100.0) as u32
                    } else {
                        0
                    };
                    let bar_width = 20usize;
                    let filled = if total > 0 {
                        (done as usize * bar_width / total as usize).min(bar_width)
                    } else {
                        0
                    };
                    let bar: String = format!(
                        "[{}{}] {}/{}  {}%",
                        "=".repeat(filled),
                        " ".repeat(bar_width - filled),
                        done,
                        total,
                        pct
                    );
                    lines.push(Line::styled(
                        format!("Progress: {}", bar),
                        Style::default().fg(theme::blue()),
                    ));
                }
            }

            lines.push(Line::styled(
                "[n]New [t]Task [c]Cmt [s]Status [e]Edit [</>page]",
                Style::default().fg(theme::overlay0()),
            ));

            lines
        })
        .unwrap_or_else(|| {
            vec![Line::styled(
                " Select a work item",
                Style::default().fg(theme::overlay0()),
            )]
        });

    f.render_widget(
        Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true }),
        area,
    );
}

/// Task list panel (bottom-right of top half) — navigable list of work_tasks
fn render_task_list(f: &mut Frame, area: Rect, app: &mut App) {
    let task_list_focused = app.work_focus == 1;
    let border_color = if task_list_focused {
        theme::blue()
    } else {
        theme::surface1()
    };

    let title = format!(" Tasks ({}) ", app.work_tasks.len());
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    if app.work_tasks.is_empty() {
        let p = Paragraph::new(" No linked tasks")
            .style(Style::default().fg(theme::overlay0()))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .work_tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let icon = state_badge::task_state_icon(&t.state);
            let style = state_badge::task_state_style(&t.state);
            let is_selected = task_list_focused && app.work_task_selected == Some(i);

            let agent_tag = t
                .assigned_agent_id
                .and_then(|aid| {
                    app.agents.iter().find(|a| a.id == aid).map(|a| {
                        let short: String = a.name.chars().filter(|c| c.is_uppercase()).collect();
                        format!(" [{}]", short)
                    })
                })
                .unwrap_or_default();

            let cost_str = if t.cost_usd > 0.0 {
                format!(" ${:.2}", t.cost_usd)
            } else {
                String::new()
            };

            let row_style = if is_selected {
                Style::default().fg(theme::base()).bg(theme::blue())
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", icon),
                    if is_selected { row_style } else { style },
                ),
                Span::styled(
                    format!("#{} ", t.number),
                    if is_selected {
                        row_style
                    } else {
                        Style::default().fg(theme::overlay1())
                    },
                ),
                Span::styled(
                    format!("{:9} ", t.state),
                    if is_selected { row_style } else { style },
                ),
                Span::styled(
                    t.title.clone(),
                    if is_selected {
                        row_style
                    } else {
                        Style::default().fg(theme::text())
                    },
                ),
                Span::styled(
                    agent_tag,
                    if is_selected {
                        row_style
                    } else {
                        Style::default().fg(theme::mauve())
                    },
                ),
                Span::styled(
                    cost_str,
                    if is_selected {
                        row_style
                    } else {
                        Style::default().fg(theme::subtext0())
                    },
                ),
            ]))
        })
        .collect();

    app.work_task_list_state.select(app.work_task_selected);
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(theme::base()).bg(theme::blue()));
    f.render_stateful_widget(list, area, &mut app.work_task_list_state);
}

/// Bottom half: full task detail for the selected work task
fn render_task_detail(f: &mut Frame, area: Rect, app: &mut App) {
    let task_detail_focused = app.work_focus == 1;
    let border_color = if task_detail_focused {
        theme::blue()
    } else {
        theme::surface1()
    };

    let block = Block::default()
        .title(" Task Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let task = app.work_task_selected.and_then(|i| app.work_tasks.get(i));

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

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("#{} ", t.number),
                    Style::default().fg(theme::overlay1()),
                ),
                Span::styled(t.title.clone(), Style::default().fg(theme::text())),
            ]),
            Line::from(vec![
                Span::styled("Kind: ", Style::default().fg(theme::subtext0())),
                Span::styled(
                    if t.kind.is_empty() { "—" } else { &t.kind },
                    Style::default().fg(theme::text()),
                ),
                Span::styled("  State: ", Style::default().fg(theme::subtext0())),
                Span::styled(t.state.clone(), state_badge::task_state_style(&t.state)),
                if t.urgent {
                    Span::styled("  URGENT", Style::default().fg(theme::red()))
                } else {
                    Span::styled("", Style::default())
                },
            ]),
            Line::from(vec![
                Span::styled("Agent: ", Style::default().fg(theme::subtext0())),
                Span::styled(agent_name, Style::default().fg(theme::mauve())),
                Span::styled(
                    format!("  Cost: ${:.4}", t.cost_usd),
                    Style::default().fg(theme::subtext0()),
                ),
                if t.flagged {
                    Span::styled("  Flagged", Style::default().fg(theme::yellow()))
                } else {
                    Span::styled("", Style::default())
                },
            ]),
        ];

        lines.push(Line::from(""));

        // Spec
        if let Some(spec) = t.context.get("spec").and_then(|s| s.as_str()) {
            if !spec.is_empty() {
                lines.push(Line::styled(
                    "── Spec ──",
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

        // Files
        if let Some(files) = t.context.get("files").and_then(|f| f.as_array()) {
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

        // Acceptance criteria
        if let Some(acceptance) = t.context.get("acceptance").and_then(|a| a.as_array()) {
            if !acceptance.is_empty() {
                lines.push(Line::styled(
                    "── Acceptance ──",
                    Style::default().fg(theme::surface1()),
                ));
                for criterion in acceptance {
                    if let Some(text) = criterion.as_str() {
                        lines.push(Line::styled(
                            format!("  {}", text),
                            Style::default().fg(theme::text()),
                        ));
                    }
                }
                lines.push(Line::from(""));
            }
        }

        // Updates
        lines.push(Line::styled(
            "── Updates ──",
            Style::default().fg(theme::surface1()),
        ));

        if app.work_task_updates.is_empty() {
            lines.push(Line::styled(
                " No updates yet",
                Style::default().fg(theme::overlay0()),
            ));
        } else {
            for u in &app.work_task_updates {
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
        }

        // Comments
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "── Comments ──",
            Style::default().fg(theme::surface1()),
        ));

        if app.work_task_comments.is_empty() {
            lines.push(Line::styled(
                " No comments yet",
                Style::default().fg(theme::overlay0()),
            ));
        } else {
            for c in &app.work_task_comments {
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
        }

        lines
    } else {
        vec![Line::styled(
            " Select a task to view details (Tab to switch, ↑↓ to navigate)",
            Style::default().fg(theme::overlay0()),
        )]
    };

    // Clamp scroll
    let content_len = content.len() as u16;
    let inner_height = area.height.saturating_sub(2);
    if content_len <= inner_height {
        app.work_task_detail_scroll = 0;
    } else if app.work_task_detail_scroll > content_len.saturating_sub(inner_height) {
        app.work_task_detail_scroll = content_len.saturating_sub(inner_height);
    }

    let p = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.work_task_detail_scroll, 0));
    f.render_widget(p, area);
}
