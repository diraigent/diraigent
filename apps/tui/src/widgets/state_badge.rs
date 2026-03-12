use crate::theme;
use ratatui::style::{Color, Style};

pub fn task_state_style(state: &str) -> Style {
    let color = match state {
        "ready" => theme::green(),
        "done" => theme::overlay0(),
        "cancelled" => theme::red(),
        "backlog" => theme::subtext0(),
        // Any active step name (implement, review, merge, working, etc.)
        _ => theme::blue(),
    };
    Style::default().fg(color)
}

pub fn task_state_icon(state: &str) -> &'static str {
    match state {
        "ready" => "○",
        "done" => "✓",
        "cancelled" => "✗",
        "backlog" => "·",
        // Any active step name
        _ => "●",
    }
}

pub fn update_kind_color(kind: &str) -> Color {
    match kind {
        "blocker" => theme::red(),
        "question" => theme::yellow(),
        "artifact" => theme::teal(),
        "progress" => theme::blue(),
        _ => theme::text(),
    }
}
