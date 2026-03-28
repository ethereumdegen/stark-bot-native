mod app;
mod cli;
mod commands;
mod config;
mod db;
mod event;
mod starflask;
mod theme;
mod ui;

use std::io;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, Mode, Screen};
use cli::{Cli, Commands};
use event::{AppEvent, EventHandler};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Dispatch subcommands (non-TUI)
    match cli.command {
        Some(Commands::Agents) => {
            if let Err(e) = commands::agents::run() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Provision { file }) => {
            let cfg = config::Config::load();
            let _ = cfg; // ensure .env is loaded
            if let Err(e) = commands::provision::run(file).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Query { agent, message }) => {
            let cfg = config::Config::load();
            let _ = cfg;
            if let Err(e) = commands::query::run(&agent, &message).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Config { key, value }) => {
            if let Err(e) = commands::config_cmd::run(key, value) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        None => {} // Launch TUI
    }

    // ── TUI mode ──
    if let Err(e) = run_tui().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run_tui() -> Result<(), String> {
    let cfg = config::Config::load();
    let db = db::Database::open(&config::db_path())?;
    let mut app = App::new(cfg, db);

    // Load agents from DB
    app.load_agents();

    // Check connection
    if app.client.is_none() {
        app.push_system("Warning: STARFLASK_API_KEY not set. Set it in ~/.config/starkbot/.env");
    }
    if app.agents.is_empty() {
        app.push_system("No agents loaded. Run `starkbot provision` to sync agents.");
    }

    // Setup terminal
    enable_raw_mode().map_err(|e| e.to_string())?;
    io::stdout().execute(EnterAlternateScreen).map_err(|e| e.to_string())?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;

    // Event handler
    let mut events = EventHandler::new(Duration::from_millis(250));
    app.set_event_tx(events.tx());

    // Main loop
    while app.running {
        terminal.draw(|frame| ui::render(frame, &app)).map_err(|e| e.to_string())?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => handle_key(&mut app, key),
                AppEvent::Tick => {} // just redraw
                AppEvent::Progress(evt) => app.handle_progress(evt),
                AppEvent::QueryComplete(result) => app.handle_query_complete(result),
            }
        }
    }

    // Restore terminal
    disable_raw_mode().map_err(|e| e.to_string())?;
    io::stdout().execute(LeaveAlternateScreen).map_err(|e| e.to_string())?;
    Ok(())
}

fn handle_key(app: &mut App, key: crossterm::event::KeyEvent) {
    // Ctrl+C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.running = false;
        return;
    }

    match app.mode {
        Mode::Normal => handle_normal_key(app, key),
        Mode::Insert => handle_insert_key(app, key),
    }
}

fn handle_normal_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match app.screen {
        Screen::Chat => match key.code {
            KeyCode::Char('q') => app.running = false,
            KeyCode::Char('i') => app.mode = Mode::Insert,
            KeyCode::Char('?') => app.screen = Screen::Help,
            KeyCode::Tab => app.screen = Screen::Agents,
            KeyCode::Char('j') | KeyCode::Down => {
                app.auto_scroll = false;
                app.scroll_offset = app.scroll_offset.saturating_add(1)
                    .min(app.messages.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.auto_scroll = false;
                app.scroll_offset = app.scroll_offset.saturating_sub(1);
            }
            KeyCode::Char('g') => {
                app.auto_scroll = false;
                app.scroll_offset = 0;
            }
            KeyCode::Char('G') => {
                app.auto_scroll = true;
                app.scroll_to_bottom();
            }
            _ => {}
        },
        Screen::Agents => match key.code {
            KeyCode::Esc | KeyCode::Tab => app.screen = Screen::Chat,
            KeyCode::Char('q') => app.running = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if app.agent_selection < app.agents.len().saturating_sub(1) {
                    app.agent_selection += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.agent_selection = app.agent_selection.saturating_sub(1);
            }
            KeyCode::Enter => {
                let idx = app.agent_selection;
                app.select_agent(idx);
            }
            _ => {}
        },
        Screen::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => app.screen = Screen::Chat,
            _ => {}
        },
    }
}

fn handle_insert_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Enter => {
            if !app.querying {
                app.submit_query();
            }
        }
        KeyCode::Backspace => app.input_backspace(),
        KeyCode::Delete => app.input_delete(),
        KeyCode::Left => app.input_left(),
        KeyCode::Right => app.input_right(),
        KeyCode::Home => app.input_home(),
        KeyCode::End => app.input_end(),
        KeyCode::Char(c) => app.input_char(c),
        _ => {}
    }
}
