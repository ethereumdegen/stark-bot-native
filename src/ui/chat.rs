use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let lines: Vec<Line> = app.messages.iter().map(|msg| {
        let (prefix, style) = match msg.role.as_str() {
            "user" => ("you> ", theme.user_msg),
            "agent" => (format!("{}> ", app.current_agent).leak() as &str, theme.agent_msg),
            "system" => ("--- ", theme.system_msg),
            "progress" => ("... ", theme.progress),
            "error" => ("ERR ", theme.error),
            _ => ("    ", theme.system_msg),
        };
        Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(&msg.content, style),
        ])
    }).collect();

    let total_lines = lines.len();
    let visible_height = area.height.saturating_sub(2) as usize; // minus borders

    // Auto-scroll: show the latest messages
    let scroll = if app.auto_scroll {
        total_lines.saturating_sub(visible_height)
    } else {
        app.scroll_offset
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" chat "))
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));

    frame.render_widget(paragraph, area);
}
