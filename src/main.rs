mod app;
mod cli;
mod commands;
mod config;
mod db;
mod event;
mod render;
mod starflask;
mod theme;

use std::time::Duration;

use clap::Parser;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use app::{App, Screen, SlashResult};
use cli::{Cli, Commands};
use event::{AppEvent, EventHandler};
use render::InlineRenderer;

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
            let _ = cfg;
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
        None => {}
    }

    if let Err(e) = run_tui().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run_tui() -> Result<(), String> {
    let cfg = config::Config::load();
    let db = db::Database::open(&config::db_path())?;
    let mut app = App::new(cfg, db);
    app.load_agents();

    let mut renderer = InlineRenderer::new();

    // Panic hook: restore terminal
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        default_hook(info);
    }));

    // Print splash
    renderer.print_splash(&app.current_agent, app.client.is_some());

    // Setup flow if needed
    if app.screen == Screen::Setup {
        renderer.print_setup_prompt();
    }

    // Enter raw mode
    enable_raw_mode().map_err(|e| e.to_string())?;

    // Event handler
    let mut events = EventHandler::new(Duration::from_millis(150));
    app.set_event_tx(events.tx());

    // Draw initial prompt if in chat mode
    if app.screen == Screen::Chat {
        renderer.redraw_input("you> ", &app.input, app.cursor_pos);
    }

    while app.running {
        let Some(event) = events.next().await else { break };

        match event {
            AppEvent::Key(key) => {
                // Ctrl+C always quits
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.running = false;
                    break;
                }

                match app.screen {
                    Screen::Setup => handle_setup_key(&mut app, &mut renderer, key),
                    Screen::Chat => handle_chat_key(&mut app, &mut renderer, key),
                }
            }
            AppEvent::Tick => {}
            AppEvent::Progress(evt) => {
                app.handle_progress(evt);
                if let Some(ref text) = app.progress_text {
                    renderer.update_progress(text);
                }
            }
            AppEvent::QueryComplete(result) => {
                renderer.clear_progress();
                let content = app.handle_query_complete(result);
                let agent = app.current_agent.clone();
                if content.starts_with("Error:") {
                    renderer.print_error(&content);
                } else {
                    renderer.print_agent_message(&agent, &content);
                }
                renderer.redraw_input("you> ", &app.input, app.cursor_pos);
            }
        }
    }

    // Restore terminal
    disable_raw_mode().map_err(|e| e.to_string())?;
    renderer.newline();
    Ok(())
}

fn handle_setup_key(app: &mut App, renderer: &mut InlineRenderer, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            renderer.newline();
            renderer.print_system_message("Skipping setup — running in disconnected mode");
            app.screen = Screen::Chat;
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::Enter => {
            let api_key = app.setup_input.trim().to_string();
            if !api_key.is_empty() {
                renderer.newline();
                if let Err(e) = config::Config::save_api_key(&api_key) {
                    renderer.print_error(&format!("Failed to save key: {}", e));
                }
                app.finish_setup();
                renderer.print_system_message("API key saved");
                renderer.redraw_input("you> ", &app.input, app.cursor_pos);
            }
        }
        KeyCode::Backspace => {
            if app.setup_cursor > 0 {
                let prev = app.setup_input[..app.setup_cursor]
                    .chars().last().map(|c| c.len_utf8()).unwrap_or(0);
                app.setup_cursor -= prev;
                app.setup_input.remove(app.setup_cursor);
                // Redraw masked input
                let masked: String = "*".repeat(app.setup_input.len());
                renderer.redraw_input("> ", &masked, app.setup_cursor);
            }
        }
        KeyCode::Char(c) => {
            app.setup_input.insert(app.setup_cursor, c);
            app.setup_cursor += c.len_utf8();
            let masked: String = "*".repeat(app.setup_input.len());
            renderer.redraw_input("> ", &masked, app.setup_cursor);
        }
        _ => {}
    }
}

fn handle_chat_key(app: &mut App, renderer: &mut InlineRenderer, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            if app.querying { return; }
            let text = app.input.trim().to_string();
            if text.is_empty() { return; }

            if text.starts_with('/') {
                // Clear input line first
                let input_copy = app.input.clone();
                app.input.clear();
                app.cursor_pos = 0;

                // Temporarily leave the prompt line
                renderer.newline();

                let result = app.handle_slash_command(&input_copy);
                match result {
                    SlashResult::Help(text) => renderer.print_help(&text),
                    SlashResult::Agents => {
                        let agents = app.agents.clone();
                        let current = app.current_agent.clone();
                        renderer.print_agents(&agents, &current);
                    }
                    SlashResult::Switched(msg) => renderer.print_system_message(&msg),
                    SlashResult::Connect => {
                        app.setup_input.clear();
                        app.setup_cursor = 0;
                        app.screen = Screen::Setup;
                        renderer.print_setup_prompt();
                        return;
                    }
                    SlashResult::Clear => renderer.clear_screen(),
                    SlashResult::Quit => {
                        app.running = false;
                        return;
                    }
                    SlashResult::Unknown(msg) => renderer.print_error(&msg),
                }
                renderer.redraw_input("you> ", &app.input, app.cursor_pos);
            } else {
                // Normal message: print it, then submit
                renderer.clear_progress();
                renderer.newline();
                renderer.print_user_message(&text);
                app.submit_query();
                // Progress will be shown via events
            }
        }
        KeyCode::Backspace => {
            app.input_backspace();
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::Delete => {
            app.input_delete();
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::Left => {
            app.input_left();
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::Right => {
            app.input_right();
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::Home => {
            app.input_home();
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::End => {
            app.input_end();
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        KeyCode::Char(c) => {
            app.input_char(c);
            renderer.redraw_input("you> ", &app.input, app.cursor_pos);
        }
        _ => {}
    }
}
