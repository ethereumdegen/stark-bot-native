use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Mode};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let mode_label = match app.mode {
        Mode::Insert => " INSERT ",
        Mode::Normal => " NORMAL ",
    };

    let title = format!(" {} ", mode_label.trim());

    let input_line = if app.mode == Mode::Insert {
        // Show cursor in input
        let before = &app.input[..app.cursor_pos];
        let cursor_char = app.input[app.cursor_pos..].chars().next().unwrap_or(' ');
        let after_start = app.cursor_pos + cursor_char.len_utf8().min(app.input.len() - app.cursor_pos);
        let after = if after_start <= app.input.len() { &app.input[after_start..] } else { "" };

        Line::from(vec![
            Span::styled(before, app.theme.input),
            Span::styled(cursor_char.to_string(), app.theme.input_cursor),
            Span::styled(after, app.theme.input),
        ])
    } else {
        Line::from(Span::styled(&app.input, app.theme.input))
    };

    let paragraph = Paragraph::new(input_line)
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(paragraph, area);
}
