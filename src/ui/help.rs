use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

const KEYBINDINGS: &[(&str, &str)] = &[
    ("i", "Enter insert mode (type messages)"),
    ("Esc", "Back to normal mode / close overlay"),
    ("Enter", "Send message (in insert mode)"),
    ("q", "Quit (in normal mode)"),
    ("Tab", "Switch to agents screen"),
    ("?", "Show this help"),
    ("j / Down", "Scroll down"),
    ("k / Up", "Scroll up"),
    ("g", "Scroll to top"),
    ("G", "Scroll to bottom"),
];

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled("Keybindings", theme.agent_name)),
        Line::from(""),
    ];

    for (key, desc) in KEYBINDINGS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<12}", key), theme.help_key),
            Span::styled(*desc, theme.help_desc),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Esc to close", theme.system_msg
    )));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" help "));

    frame.render_widget(paragraph, area);
}
