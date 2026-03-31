use crate::config::Config;
use crate::db;
use crate::starflask::StarflaskClient;

pub async fn run() -> Result<(), String> {
    let config = Config::load();
    let client = StarflaskClient::new(&config)?;
    let remote_agents = client.list_agents().await?;
    let agents = db::parse_agents(&remote_agents);

    if agents.is_empty() {
        println!("No agents found. Create agents at https://starflask.com");
        return Ok(());
    }

    println!("{:<16} {:<36} {:<24} {}", "CAPABILITY", "AGENT_ID", "NAME", "STATUS");
    println!("{}", "-".repeat(80));
    for agent in &agents {
        println!("{:<16} {:<36} {:<24} {}", agent.capability, agent.agent_id, agent.name, agent.status);
    }
    Ok(())
}
