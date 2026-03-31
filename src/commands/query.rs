use crate::config::Config;
use crate::db;
use crate::starflask::{self, StarflaskClient};

pub async fn run(agent: &str, message: &str) -> Result<(), String> {
    let config = Config::load();
    let client = StarflaskClient::new(&config)?;

    // Fetch agents from API and find the requested one
    let remote_agents = client.list_agents().await?;
    let agents = db::parse_agents(&remote_agents);
    let agent_lower = agent.to_lowercase();
    let agent_record = agents.iter()
        .find(|a| a.capability.to_lowercase() == agent_lower || a.name.to_lowercase() == agent_lower)
        .ok_or_else(|| format!("Agent '{}' not found. Available: {}", agent,
            agents.iter().map(|a| a.capability.as_str()).collect::<Vec<_>>().join(", ")))?;

    eprintln!("Querying {} ({})...", agent_record.name, agent_record.capability);

    let session_id = client.create_session(&agent_record.agent_id, message).await?;

    let result = client.poll_session(&agent_record.agent_id, &session_id, |evt| {
        match evt {
            starflask::ProgressEvent::StatusChange(s) => eprintln!("  status: {}", s),
            starflask::ProgressEvent::LogEntry { summary, .. } => eprintln!("  {}", summary),
            starflask::ProgressEvent::Error(e) => eprintln!("  warning: {}", e),
        }
    }).await?;

    let text = starflask::parse_text_result(&result.result);
    println!("{}", text);

    Ok(())
}
