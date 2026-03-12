use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, LOG_DIRECTIONS, LOG_LIMITS, TIME_RANGES};
use crate::theme;

/// Detect log level from a line. Returns the color for that level.
fn level_color(line: &str) -> ratatui::style::Color {
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("err]") || lower.contains("level=error") {
        theme::red()
    } else if lower.contains("warn") || lower.contains("wrn]") || lower.contains("level=warn") {
        theme::yellow()
    } else if lower.contains("info") || lower.contains("inf]") || lower.contains("level=info") {
        theme::blue()
    } else if lower.contains("debug") || lower.contains("dbg]") || lower.contains("level=debug") {
        theme::overlay0()
    } else {
        theme::text()
    }
}

/// Format a nanosecond timestamp to a human-readable string.
fn format_timestamp(ts: &str) -> String {
    if let Ok(nanos) = ts.parse::<u128>() {
        let secs = (nanos / 1_000_000_000) as i64;
        if let Some(dt) = chrono::DateTime::from_timestamp(secs, (nanos % 1_000_000_000) as u32) {
            return dt.format("%H:%M:%S%.3f").to_string();
        }
    }
    // Fallback: return first 19 chars or the whole string
    ts.get(..19).unwrap_or(ts).to_string()
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Controls bar
            Constraint::Min(5),    // Log output
            Constraint::Length(1), // Status line
        ])
        .split(area);

    render_controls(f, chunks[0], app);
    render_log_output(f, chunks[1], app);
    render_status(f, chunks[2], app);
}

fn render_controls(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Logs ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::teal()));

    let time_label = TIME_RANGES[app.log_time_range_idx].0;
    let limit = LOG_LIMITS[app.log_limit_idx];
    let direction = LOG_DIRECTIONS[app.log_direction_idx];

    let query_display = if app.log_query.is_empty() {
        "<press Enter to edit query>".to_string()
    } else {
        app.log_query.clone()
    };

    let filter_display = if app.log_filter.is_empty() {
        String::new()
    } else {
        format!("  filter: {}", app.log_filter)
    };

    let line = Line::from(vec![
        Span::styled(" Query: ", Style::default().fg(theme::subtext0())),
        Span::styled(&query_display, Style::default().fg(theme::text())),
        Span::styled("  │ ", Style::default().fg(theme::surface1())),
        Span::styled("Range: ", Style::default().fg(theme::subtext0())),
        Span::styled(
            format!("◀ {} ▶", time_label),
            Style::default().fg(theme::peach()),
        ),
        Span::styled("  │ ", Style::default().fg(theme::surface1())),
        Span::styled("Limit: ", Style::default().fg(theme::subtext0())),
        Span::styled(
            format!("◀ {} ▶", limit),
            Style::default().fg(theme::peach()),
        ),
        Span::styled("  │ ", Style::default().fg(theme::surface1())),
        Span::styled("Dir: ", Style::default().fg(theme::subtext0())),
        Span::styled(
            format!("◀ {} ▶", direction),
            Style::default().fg(theme::peach()),
        ),
        Span::styled(&filter_display, Style::default().fg(theme::yellow())),
    ]);

    f.render_widget(Paragraph::new(line).block(block), area);
}

fn render_log_output(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::surface1()));

    // Build lines from filtered entries. We inline the filter logic to avoid
    // holding a borrow on `app` while we later mutate `app.log_scroll`.
    let filter_lower = app.log_filter.to_lowercase();
    let lines: Vec<Line> = app
        .log_entries
        .iter()
        .filter(|e| filter_lower.is_empty() || e.line.to_lowercase().contains(&filter_lower))
        .map(|entry| {
            let ts = format_timestamp(&entry.timestamp);
            let color = level_color(&entry.line);

            // Extract app label if present for context
            let app_label = entry
                .labels
                .get("app")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mut spans = vec![Span::styled(
                format!("{} ", ts),
                Style::default().fg(theme::subtext0()),
            )];

            if !app_label.is_empty() {
                spans.push(Span::styled(
                    format!("[{}] ", app_label),
                    Style::default().fg(theme::mauve()),
                ));
            }

            spans.push(Span::styled(
                entry.line.to_string(),
                Style::default().fg(color),
            ));

            Line::from(spans)
        })
        .collect();

    if lines.is_empty() {
        let msg = if app.log_loading {
            " Loading..."
        } else if app.log_entries.is_empty() {
            " No log entries. Press Enter to submit query."
        } else {
            " No entries match filter."
        };
        let p =
            Paragraph::new(Line::styled(msg, Style::default().fg(theme::overlay0()))).block(block);
        f.render_widget(p, area);
        return;
    }

    let content_len = lines.len() as u16;
    let inner_height = area.height.saturating_sub(2);
    if content_len <= inner_height {
        app.log_scroll = 0;
    } else if app.log_scroll > content_len.saturating_sub(inner_height) {
        app.log_scroll = content_len.saturating_sub(inner_height);
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.log_scroll, 0)),
        area,
    );
}

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let total_count = app.log_entries.len();
    let filtered_count = if app.log_filter.is_empty() {
        total_count
    } else {
        let q = app.log_filter.to_lowercase();
        app.log_entries
            .iter()
            .filter(|e| e.line.to_lowercase().contains(&q))
            .count()
    };

    let status_text = if app.log_loading {
        " ⏳ Loading logs...".to_string()
    } else if !app.log_filter.is_empty() {
        format!(
            " {} of {} entries (filtered) │ Enter: query │ /: filter │ T/Y: range │ N/M: limit │ B: direction",
            filtered_count, total_count
        )
    } else {
        format!(
            " {} entries │ Enter: query │ /: filter │ T/Y: range │ N/M: limit │ B: direction",
            total_count
        )
    };

    let p = Paragraph::new(Line::styled(
        status_text,
        Style::default().fg(theme::overlay0()),
    ))
    .style(Style::default().bg(theme::mantle()));

    f.render_widget(p, area);
}
