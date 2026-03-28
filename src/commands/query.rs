use crate::config::{self, Config};
use crate::db::Database;
use crate::starflask::{self, StarflaskClient};

pub async fn run(agent: &str, message: &str) -> Result<(), String> {
    let config = Config::load();
    let db = Database::open(&config::db_path())?;
    let client = StarflaskClient::new(&config)?;

    // Resolve agent ID
    let agent_record = db.get_agent(agent)?
        .ok_or_else(|| format!("Agent '{}' not found. Run `starkbot provision` first.", agent))?;

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
