use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: i64,
    pub capability: String,
    pub agent_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Parse agents from Starflask API JSON response into in-memory Agent structs.
pub fn parse_agents(remote: &[serde_json::Value]) -> Vec<Agent> {
    let mut agents = Vec::new();
    for (i, agent) in remote.iter().enumerate() {
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

        agents.push(Agent {
            id: i as i64,
            capability: capability.to_string(),
            agent_id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
            status: "active".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        });
    }
    agents
}
