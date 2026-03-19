use crate::app::{App, SETTINGS_FIELD_COUNT};
use crate::theme;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let Some(ref form) = app.settings_form else {
        // No form loaded yet — show a loading message
        let block = Block::default()
            .title(" Project Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::mauve()))
            .style(Style::default().bg(theme::base()));
        let p = Paragraph::new(Line::styled(
            "  Loading settings…  Press Esc to go back.",
            Style::default().fg(theme::subtext0()),
        ))
        .block(block);
        f.render_widget(p, area);
        return;
    };

    // Split: left = project properties, right = CLAUDE.md
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    // ── Left: project properties ──
    let left_focused = form.active_field < 6;
    let left_block = Block::default()
        .title(" Project Properties ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if left_focused {
            theme::mauve()
        } else {
            theme::surface1()
        }))
        .style(Style::default().bg(theme::base()));
    let left_inner = left_block.inner(chunks[0]);
    f.render_widget(left_block, chunks[0]);

    let field_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Name (field 0)
            Constraint::Length(2), // Description (field 1)
            Constraint::Length(2), // Repo URL (field 2)
            Constraint::Length(2), // Repo Path (field 3)
            Constraint::Length(2), // Default Branch (field 4)
            Constraint::Length(2), // Service Name (field 5)
            Constraint::Min(1),    // Footer hint
        ])
        .split(left_inner);

    let render_field = |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
        let label_style = if active {
            Style::default().fg(theme::blue())
        } else {
            Style::default().fg(theme::subtext0())
        };
        let val_style = if active {
            Style::default().fg(theme::text())
        } else {
            Style::default().fg(theme::overlay0())
        };
        let p = Paragraph::new(vec![Line::from(vec![
            Span::styled(format!(" {} ", label), label_style),
            Span::styled(value.to_string(), val_style),
        ])]);
        f.render_widget(p, area);
    };

    // Render text field with cursor
    let render_text_field =
        |f: &mut Frame, area: Rect, label: &str, text: &str, active: bool, cursor: usize| {
            let display = if active {
                let bp = text
                    .char_indices()
                    .nth(cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());
                if bp < text.len() {
                    format!("{}│{}", &text[..bp], &text[bp..])
                } else {
                    format!("{}│", text)
                }
            } else {
                text.to_string()
            };
            render_field(f, area, label, &display, active);
        };

    // Field 0: Name
    render_text_field(
        f,
        field_chunks[0],
        "Name:",
        &form.name,
        form.active_field == 0,
        form.cursor,
    );

    // Field 1: Description
    render_text_field(
        f,
        field_chunks[1],
        "Description:",
        &form.description,
        form.active_field == 1,
        form.cursor,
    );

    // Field 2: Repo URL
    render_text_field(
        f,
        field_chunks[2],
        "Repo URL:",
        &form.repo_url,
        form.active_field == 2,
        form.cursor,
    );

    // Field 3: Repo Path
    render_text_field(
        f,
        field_chunks[3],
        "Repo Path:",
        &form.repo_path,
        form.active_field == 3,
        form.cursor,
    );

    // Field 4: Default Branch
    render_text_field(
        f,
        field_chunks[4],
        "Branch:",
        &form.default_branch,
        form.active_field == 4,
        form.cursor,
    );

    // Field 5: Service Name
    render_text_field(
        f,
        field_chunks[5],
        "Service:",
        &form.service_name,
        form.active_field == 5,
        form.cursor,
    );

    // Footer hint
    {
        let dirty_marker = if form.dirty { " [modified]" } else { "" };
        let hint = Paragraph::new(Line::styled(
            format!(" Tab: next | Ctrl+S: save | Esc: back{}", dirty_marker),
            Style::default().fg(if form.dirty {
                theme::yellow()
            } else {
                theme::overlay0()
            }),
        ));
        f.render_widget(hint, field_chunks[6]);
    }

    // ── Right: CLAUDE.md ──
    let right_focused = form.active_field == 6;
    let right_block = Block::default()
        .title(" CLAUDE.md ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if right_focused {
            theme::mauve()
        } else {
            theme::surface1()
        }))
        .style(Style::default().bg(theme::base()));
    let right_inner = right_block.inner(chunks[1]);
    f.render_widget(right_block, chunks[1]);

    // Split right panel: content + footer
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(right_inner);

    // CLAUDE.md content with cursor if active
    {
        let display = if right_focused {
            let bp = form
                .claude_md
                .char_indices()
                .nth(form.cursor)
                .map(|(i, _)| i)
                .unwrap_or(form.claude_md.len());
            if bp < form.claude_md.len() {
                format!("{}│{}", &form.claude_md[..bp], &form.claude_md[bp..])
            } else {
                format!("{}│", &form.claude_md)
            }
        } else {
            form.claude_md.clone()
        };

        // Calculate scroll position
        let area_h = right_chunks[0].height as usize;
        let avail_w = right_chunks[0].width.max(1) as usize;
        let chars_before = form.cursor;
        let cursor_line = if avail_w > 0 {
            // Approximate: count newlines up to cursor position
            let bp = form
                .claude_md
                .char_indices()
                .nth(chars_before)
                .map(|(i, _)| i)
                .unwrap_or(form.claude_md.len());
            form.claude_md[..bp].matches('\n').count()
        } else {
            0
        };
        let max_line = area_h.saturating_sub(1);
        let scroll_y = cursor_line.saturating_sub(max_line) as u16;

        let p = Paragraph::new(display)
            .style(Style::default().fg(if right_focused {
                theme::text()
            } else {
                theme::overlay0()
            }))
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0));
        f.render_widget(p, right_chunks[0]);
    }

    // Right footer
    {
        let lines = form.claude_md.lines().count();
        let hint = Paragraph::new(Line::styled(
            format!(
                " {} lines | Enter: newline | field {}/{}",
                lines,
                form.active_field + 1,
                SETTINGS_FIELD_COUNT
            ),
            Style::default().fg(theme::overlay0()),
        ));
        f.render_widget(hint, right_chunks[1]);
    }
}
