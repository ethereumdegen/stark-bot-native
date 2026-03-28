use crate::config::{self, Config};
use crate::db::Database;
use crate::starflask::StarflaskClient;

pub async fn run(file: Option<String>) -> Result<(), String> {
    let config = Config::load();
    let db = Database::open(&config::db_path())?;

    // If a seed file is given, load agents from it
    if let Some(path) = file {
        return load_seed_file(&db, &path);
    }

    // Otherwise, sync from Starflask API
    let client = StarflaskClient::new(&config)?;
    println!("Syncing agents from Starflask...");

    let remote_agents = client.list_agents().await?;
    let mut count = 0;

    for agent in &remote_agents {
        let id = agent.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let desc = agent.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let capability = agent.get("capability")
            .or_else(|| agent.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if id.is_empty() || capability.is_empty() {
            continue;
        }

        db.upsert_agent(capability, id, name, desc, "active")?;
        println!("  {} ({}) -> {}", capability, name, id);
        count += 1;
    }

    println!("Synced {} agents.", count);
    Ok(())
}

fn load_seed_file(db: &Database, path: &str) -> Result<(), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;
    let agents: Vec<serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path, e))?;

    let mut count = 0;
    for agent in &agents {
        let id = agent.get("agent_id").or_else(|| agent.get("id"))
            .and_then(|v| v.as_str()).unwrap_or("");
        let capability = agent.get("capability").and_then(|v| v.as_str()).unwrap_or("");
        let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let desc = agent.get("description").and_then(|v| v.as_str()).unwrap_or("");

        if id.is_empty() || capability.is_empty() { continue; }

        db.upsert_agent(capability, id, name, desc, "active")?;
        println!("  {} ({}) -> {}", capability, name, id);
        count += 1;
    }

    println!("Loaded {} agents from seed file.", count);
    Ok(())
}
