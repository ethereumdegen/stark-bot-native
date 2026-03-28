use std::io::{self, BufRead, Write};

use crate::config::{self, Config};
use crate::db::Database;
use crate::starflask::StarflaskClient;
use crate::theme::*;

pub async fn run() -> Result<(), String> {
    println!("\n{BOLD}{CYAN}starkbot setup{RESET}\n");

    // ── Step 1: API Key ──
    println!("{BOLD}[1/3] API Key{RESET}");

    let cfg = Config::load();
    let has_key = cfg.api_key().is_some();
    if has_key {
        println!("  {GREEN}API key already configured.{RESET}");
        print!("  Replace it? [y/N] ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let answer = read_line()?;
        if answer.trim().eq_ignore_ascii_case("y") {
            prompt_api_key()?;
        }
    } else {
        println!("  No API key found.");
        prompt_api_key()?;
    }

    // Reload config after potential key change
    let cfg = Config::load();
    if cfg.api_key().is_none() {
        println!("\n{YELLOW}No API key set — skipping remaining steps.{RESET}");
        println!("{GRAY}Run `starkbot setup` again when you have a key.{RESET}");
        return Ok(());
    }

    // ── Step 2: Provision ──
    println!("\n{BOLD}[2/3] Provision agents{RESET}");
    println!("  Syncing agents from Starflask...");

    let client = StarflaskClient::new(&cfg)?;
    let db = Database::open(&config::db_path())?;
    let remote_agents = client.list_agents().await?;
    let mut count = 0;

    for agent in &remote_agents {
        let id = agent.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let desc = agent.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let capability = agent
            .get("capability")
            .or_else(|| agent.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if id.is_empty() || capability.is_empty() {
            continue;
        }

        db.upsert_agent(capability, id, name, desc, "active")?;
        println!("  {GREEN}+{RESET} {capability} {GRAY}({name}){RESET}");
        count += 1;
    }

    if count == 0 {
        println!("  {YELLOW}No agents found on Starflask.{RESET}");
        return Ok(());
    }
    println!("  Synced {count} agent(s).");

    // ── Step 3: Select project ──
    println!("\n{BOLD}[3/3] Select project{RESET}");
    println!("  Fetching projects...");

    let projects = client.list_projects().await;
    let mut cfg = cfg;

    match projects {
        Ok(projects) if !projects.is_empty() => {
            for (i, p) in projects.iter().enumerate() {
                let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("(unnamed)");
                let marker = if cfg.project_id.as_deref() == Some(id) {
                    format!("{GREEN}*{RESET}")
                } else {
                    " ".to_string()
                };
                println!("  {marker} {BOLD}{}{RESET}  {name} {GRAY}{id}{RESET}", i + 1);
            }

            print!("\n  Pick a project [1-{}]: ", projects.len());
            io::stdout().flush().map_err(|e| e.to_string())?;
            let input = read_line()?;
            let input = input.trim();

            if let Ok(n) = input.parse::<usize>() {
                if n >= 1 && n <= projects.len() {
                    let chosen = &projects[n - 1];
                    let pid = chosen.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let pname = chosen.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    cfg.project_id = Some(pid.to_string());
                    cfg.save()?;
                    println!("  {GREEN}Project set to {BOLD}{pname}{RESET}");
                    println!("  {GRAY}Messages will be sent to the project's chat channel.{RESET}");
                } else {
                    println!("  {YELLOW}Invalid selection — keeping current setting.{RESET}");
                }
            } else if !input.is_empty() {
                println!("  {YELLOW}Invalid input — keeping current setting.{RESET}");
            } else if cfg.project_id.is_some() {
                println!("  Keeping current project.");
            } else {
                println!("  {GRAY}No project selected — will use direct agent queries.{RESET}");
            }
        }
        Ok(_) => {
            println!("  {YELLOW}No projects found.{RESET}");
            println!("  {GRAY}Messages will be sent directly to agents.{RESET}");
        }
        Err(e) => {
            println!("  {YELLOW}Could not fetch projects: {e}{RESET}");
            println!("  {GRAY}Messages will be sent directly to agents.{RESET}");
        }
    }

    println!("\n{GREEN}{BOLD}Setup complete!{RESET} Run {CYAN}starkbot{RESET} to start chatting.\n");
    Ok(())
}

fn prompt_api_key() -> Result<(), String> {
    print!("  Enter your Starflask API key: ");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let key = read_line()?;
    let key = key.trim();
    if key.is_empty() {
        println!("  {YELLOW}No key entered.{RESET}");
        return Ok(());
    }
    Config::save_api_key(key)?;
    println!("  {GREEN}API key saved.{RESET}");
    Ok(())
}

fn read_line() -> Result<String, String> {
    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line)
}
