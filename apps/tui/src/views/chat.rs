use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, CHAT_MODELS};
use crate::theme;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    // Messages area — show current model in title
    let model_name = CHAT_MODELS.get(app.chat_model_index).unwrap_or(&"sonnet");
    let msg_block = Block::default()
        .title(format!(" Chat — model: {} ", model_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::yellow()));

    let mut lines: Vec<Line> = Vec::new();
    if app.chat_messages.is_empty() && !app.chat_streaming {
        lines.push(Line::styled(
            "  Press 'i' to start typing a message...",
            Style::default().fg(theme::overlay0()),
        ));
    }
    for msg in &app.chat_messages {
        let (prefix, color) = match msg.role.as_str() {
            "user" => ("You: ", theme::blue()),
            "assistant" => ("AI: ", theme::green()),
            _ => ("", theme::text()),
        };
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![Span::styled(
            prefix,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )]));
        for line in msg.content.lines() {
            lines.push(Line::styled(
                format!("  {}", line),
                Style::default().fg(theme::text()),
            ));
        }
    }
    if app.chat_streaming {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "  Thinking...",
            Style::default().fg(theme::peach()),
        ));
    }

    // Auto-scroll: calculate visible height and set scroll
    let visible_height = chunks[0].height.saturating_sub(2) as usize;
    if lines.len() > visible_height {
        app.chat_scroll = (lines.len() - visible_height) as u16;
    }

    let messages = Paragraph::new(lines)
        .block(msg_block)
        .wrap(Wrap { trim: false })
        .scroll((app.chat_scroll, 0));
    f.render_widget(messages, chunks[0]);

    // Input area
    let input_title = if app.modal == crate::app::Modal::ChatInput {
        " Input (Enter to send, Esc to cancel) "
    } else {
        " Input (press 'i' to type) "
    };
    let input_block = Block::default()
        .title(input_title)
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(if app.modal == crate::app::Modal::ChatInput {
                theme::green()
            } else {
                theme::surface1()
            }),
        );

    let input = Paragraph::new(app.chat_input.as_str())
        .block(input_block)
        .style(Style::default().fg(theme::text()));
    f.render_widget(input, chunks[1]);
}
