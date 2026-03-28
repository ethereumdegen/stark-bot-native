use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub user_msg: Style,
    pub agent_msg: Style,
    pub system_msg: Style,
    pub status_bar: Style,
    pub input: Style,
    pub input_cursor: Style,
    pub agent_name: Style,
    pub progress: Style,
    pub error: Style,
    pub help_key: Style,
    pub help_desc: Style,
    pub table_header: Style,
    pub table_selected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            user_msg: Style::default().fg(Color::Cyan),
            agent_msg: Style::default().fg(Color::Green),
            system_msg: Style::default().fg(Color::DarkGray),
            status_bar: Style::default().fg(Color::White).bg(Color::DarkGray),
            input: Style::default().fg(Color::White),
            input_cursor: Style::default().fg(Color::Black).bg(Color::White),
            agent_name: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            progress: Style::default().fg(Color::Yellow),
            error: Style::default().fg(Color::Red),
            help_key: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            help_desc: Style::default().fg(Color::Gray),
            table_header: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            table_selected: Style::default().bg(Color::DarkGray),
        }
    }
}
