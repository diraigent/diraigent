use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(v[1])[1]
}

pub struct InputPopup<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub cursor: usize,
}

impl<'a> Widget for InputPopup<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup_area = centered_rect(60, 30, area);
        Clear.render(popup_area, buf);

        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::blue()))
            .style(Style::default().bg(theme::mantle()));
        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        let byte_pos = self
            .text
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        let display = if byte_pos < self.text.len() {
            format!("{}│{}", &self.text[..byte_pos], &self.text[byte_pos..])
        } else {
            format!("{}│", self.text)
        };

        let help = Line::styled(
            "Enter to submit, Esc to cancel",
            Style::default().fg(theme::overlay0()),
        );

        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

        Paragraph::new(display)
            .style(Style::default().fg(theme::text()))
            .render(chunks[0], buf);
        Paragraph::new(help).render(chunks[1], buf);
    }
}
