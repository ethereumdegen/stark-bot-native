use crate::config::Config;
use crate::db;
use crate::starflask::StarflaskClient;

pub async fn run(file: Option<String>) -> Result<(), String> {
    let config = Config::load();

    // If a seed file is given, load and display agents from it
    if let Some(path) = file {
        return show_seed_file(&path);
    }

    // Otherwise, sync from Starflask API
    let client = StarflaskClient::new(&config)?;
    println!("Syncing agents from Starflask...");

    let remote_agents = client.list_agents().await?;
    let agents = db::parse_agents(&remote_agents);

    for agent in &agents {
        println!("  {} ({}) -> {}", agent.capability, agent.name, agent.agent_id);
    }

    println!("Synced {} agents.", agents.len());
    Ok(())
}

fn show_seed_file(path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;
    let raw: Vec<serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path, e))?;

    let agents = db::parse_agents(&raw);
    for agent in &agents {
        println!("  {} ({}) -> {}", agent.capability, agent.name, agent.agent_id);
    }
    println!("Found {} agents in seed file.", agents.len());
    Ok(())
}
