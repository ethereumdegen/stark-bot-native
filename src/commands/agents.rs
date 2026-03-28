use crate::config;
use crate::db::Database;

pub fn run() -> Result<(), String> {
    let db = Database::open(&config::db_path())?;
    let agents = db.list_agents()?;

    if agents.is_empty() {
        println!("No agents found. Run `starkbot provision` to sync agents.");
        return Ok(());
    }

    println!("{:<16} {:<36} {:<24} {}", "CAPABILITY", "AGENT_ID", "NAME", "STATUS");
    println!("{}", "-".repeat(80));
    for agent in &agents {
        println!("{:<16} {:<36} {:<24} {}", agent.capability, agent.agent_id, agent.name, agent.status);
    }
    Ok(())
}
