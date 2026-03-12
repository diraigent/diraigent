use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // Left: branch list
    let block = Block::default()
        .title(" Branches ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 0 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let items: Vec<ListItem> = app
        .branches
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let pushed = if b.is_pushed { "↑" } else { "○" };
            let ahead_behind = if b.ahead_remote > 0 || b.behind_remote > 0 {
                format!(" +{}/-{}", b.ahead_remote, b.behind_remote)
            } else {
                String::new()
            };
            let current = if b.name == app.current_branch {
                "* "
            } else {
                "  "
            };
            let label = format!(
                "{}{} {} {}{}",
                current,
                pushed,
                b.name,
                b.commit.get(..7).unwrap_or(&b.commit),
                ahead_behind
            );
            let style = if Some(i) == app.selected_branch {
                Style::default().fg(theme::base()).bg(theme::yellow())
            } else if b.name == app.current_branch {
                Style::default().fg(theme::green())
            } else {
                Style::default().fg(theme::text())
            };
            ListItem::new(Line::styled(label, style))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, chunks[0]);

    // Right: detail + main status
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Branch detail
    let detail_block = Block::default()
        .title(" Branch Detail ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if app.focus == 1 {
            theme::yellow()
        } else {
            theme::surface1()
        }));

    let detail_text = if let Some(idx) = app.selected_branch {
        if let Some(b) = app.branches.get(idx) {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(theme::subtext0())),
                    Span::styled(&b.name, Style::default().fg(theme::blue())),
                ]),
                Line::from(vec![
                    Span::styled("Commit: ", Style::default().fg(theme::subtext0())),
                    Span::styled(&b.commit, Style::default().fg(theme::text())),
                ]),
                Line::from(vec![
                    Span::styled("Pushed: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        if b.is_pushed { "Yes" } else { "No" },
                        Style::default().fg(if b.is_pushed {
                            theme::green()
                        } else {
                            theme::peach()
                        }),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("Ahead: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        b.ahead_remote.to_string(),
                        Style::default().fg(theme::green()),
                    ),
                    Span::styled("  Behind: ", Style::default().fg(theme::subtext0())),
                    Span::styled(
                        b.behind_remote.to_string(),
                        Style::default().fg(theme::red()),
                    ),
                ]),
            ];
            if let Some(ref prefix) = b.task_id_prefix {
                lines.push(Line::from(vec![
                    Span::styled("Task: ", Style::default().fg(theme::subtext0())),
                    Span::styled(prefix, Style::default().fg(theme::mauve())),
                ]));
            }
            lines
        } else {
            vec![Line::styled(
                "No branch selected",
                Style::default().fg(theme::overlay0()),
            )]
        }
    } else {
        vec![Line::styled(
            "No branch selected",
            Style::default().fg(theme::overlay0()),
        )]
    };

    let detail = Paragraph::new(detail_text)
        .block(detail_block)
        .wrap(Wrap { trim: true });
    f.render_widget(detail, right_chunks[0]);

    // Main push status
    let main_block = Block::default()
        .title(" Main Branch Status ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    let main_text = if let Some(ref status) = app.main_push_status {
        let mut lines = vec![Line::from(vec![
            Span::styled("Ahead: ", Style::default().fg(theme::subtext0())),
            Span::styled(
                status.ahead.to_string(),
                Style::default().fg(if status.ahead > 0 {
                    theme::green()
                } else {
                    theme::text()
                }),
            ),
            Span::styled("  Behind: ", Style::default().fg(theme::subtext0())),
            Span::styled(
                status.behind.to_string(),
                Style::default().fg(if status.behind > 0 {
                    theme::red()
                } else {
                    theme::text()
                }),
            ),
        ])];
        if let Some(ref commit) = status.last_commit {
            lines.push(Line::from(vec![
                Span::styled("Last commit: ", Style::default().fg(theme::subtext0())),
                Span::styled(commit, Style::default().fg(theme::text())),
            ]));
        }
        if let Some(ref msg) = status.last_commit_message {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default().fg(theme::subtext0())),
                Span::styled(msg, Style::default().fg(theme::overlay0())),
            ]));
        }
        if let Some(ref result) = app.git_action_result {
            lines.push(Line::raw(""));
            lines.push(Line::styled(result, Style::default().fg(theme::green())));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            " [p]ush branch  [P]ush main  [R]esolve+push  [r]efresh",
            Style::default().fg(theme::overlay0()),
        ));
        lines
    } else {
        vec![Line::styled(
            "Loading...",
            Style::default().fg(theme::overlay0()),
        )]
    };

    let main_p = Paragraph::new(main_text)
        .block(main_block)
        .wrap(Wrap { trim: true });
    f.render_widget(main_p, right_chunks[1]);
}
