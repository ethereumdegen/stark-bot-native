mod chat;
mod input;
mod status_bar;
pub mod agents;
mod help;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::{App, Screen};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),        // chat area
            Constraint::Length(3),      // input
            Constraint::Length(1),      // status bar
        ])
        .split(frame.area());

    match app.screen {
        Screen::Chat => {
            chat::render(frame, app, chunks[0]);
            input::render(frame, app, chunks[1]);
            status_bar::render(frame, app, chunks[2]);
        }
        Screen::Agents => {
            agents::render(frame, app, chunks[0]);
            input::render(frame, app, chunks[1]);
            status_bar::render(frame, app, chunks[2]);
        }
        Screen::Help => {
            help::render(frame, app, chunks[0]);
            input::render(frame, app, chunks[1]);
            status_bar::render(frame, app, chunks[2]);
        }
    }
}
