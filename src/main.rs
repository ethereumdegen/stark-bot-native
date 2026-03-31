mod app;
mod cli;
mod commands;
mod config;
mod db;
mod starflask;
mod ui;

use clap::Parser;
use iocraft::prelude::*;

use cli::{Cli, Commands};
use ui::app::StarkbotApp;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Dispatch subcommands (non-TUI)
    match cli.command {
        Some(Commands::Agents) => {
            if let Err(e) = commands::agents::run().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Provision { file }) => {
            if let Err(e) = commands::provision::run(file).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Query { agent, message }) => {
            if let Err(e) = commands::query::run(&agent, &message).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Setup) => {
            if let Err(e) = commands::setup::run().await {
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

    if let Err(e) = run_tui() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_tui() -> Result<(), String> {
    let cfg = config::Config::load();

    smol::block_on(
        element! {
            StarkbotApp(
                config: Some(cfg),
            )
        }
        .render_loop()
        .fullscreen()
        .disable_mouse_capture()
        .ignore_ctrl_c(),
    )
    .map_err(|e| e.to_string())
}
