use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let connected = if app.client.is_some() { "connected" } else { "disconnected" };
    let querying = if app.querying { " | querying..." } else { "" };

    let status = Line::from(vec![
        Span::styled(
            format!(" agent: {} | {} {}", app.current_agent, connected, querying),
            app.theme.status_bar,
        ),
        Span::styled(
            " | i:insert q:quit ?:help Tab:agents ".to_string(),
            app.theme.status_bar,
        ),
    ]);

    let bar = Paragraph::new(status);
    frame.render_widget(bar, area);
}
