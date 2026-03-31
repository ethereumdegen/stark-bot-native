use crate::config::Config;
use crate::db;
use crate::starflask::StarflaskClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup,
    SetupProject,
    Chat,
}

/// Result of a slash command.
pub enum SlashResult {
    Help(String),
    Agents,
    Switched(String),
    Connect,
    Provision,
    Clear,
    Reset,
    Quit,
    Unknown(String),
    /// Async commands that need API calls.
    Tasks { status_filter: Option<String> },
    TaskCreate { title: String, description: String, priority: String },
    TaskUpdate { task_id: String, status: String },
    Schedules,
    Credits,
    History { limit: u32 },
    Memories { limit: u32 },
}

/// A single chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct App {
    pub config: Config,
    pub client: Option<StarflaskClient>,

    // Setup wizard state
    pub setup_projects: Option<Vec<(String, String)>>,

    // Agent state
    pub current_agent: String,
    pub current_agent_id: Option<String>,
    pub agents: Vec<db::Agent>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let client = StarflaskClient::new(&config).ok();
        let current_agent = config.default_agent.clone();

        Self {
            config,
            client,
            setup_projects: None,
            current_agent,
            current_agent_id: None,
            agents: Vec::new(),
        }
    }

    pub fn finish_setup(&mut self) {
        self.client = StarflaskClient::new(&self.config).ok();
    }

    fn resolve_current_agent(&mut self) {
        let target = self.current_agent.to_lowercase();
        self.current_agent_id = self.agents.iter()
            .find(|a| a.capability.to_lowercase() == target)
            .or_else(|| self.agents.iter().find(|a|
                a.capability.to_lowercase().contains(&target) ||
                a.name.to_lowercase().contains(&target)
            ))
            .or_else(|| self.agents.first())
            .map(|a| {
                self.current_agent = a.capability.clone();
                a.agent_id.clone()
            });
    }

    pub fn select_agent_by_name(&mut self, name: &str) -> Option<String> {
        let name_lower = name.to_lowercase();
        if let Some(agent) = self.agents.iter().find(|a| a.capability.to_lowercase() == name_lower || a.name.to_lowercase() == name_lower) {
            self.current_agent = agent.capability.clone();
            self.current_agent_id = Some(agent.agent_id.clone());
            let msg = format!("Switched to agent: {}", agent.name);
            Some(msg)
        } else {
            None
        }
    }

    pub fn handle_slash_command(&mut self, cmd: &str) -> SlashResult {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts[0];
        let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command {
            "/help" | "/h" => SlashResult::Help(
                "/help             Show this help\n\
                 /agents           List available agents\n\
                 /agent <name>     Switch to agent by name\n\
                 /default <name>   Set default agent (persisted)\n\
                 /tasks [status]   List project tasks (todo/in_progress/done/blocked)\n\
                 /task <title>     Create a new task\n\
                 /done <task_id>   Mark a task as done\n\
                 /schedules        List agent schedules\n\
                 /credits          Show credit balance\n\
                 /history [n]      Show recent sessions (default: 10)\n\
                 /memories [n]     Show agent memories (default: 20)\n\
                 /provision        Sync agents from Starflask API\n\
                 /connect          Set API key\n\
                 /reset            Wipe API key, config & start fresh\n\
                 /clear            Clear the screen\n\
                 /quit             Exit stark-bot"
                    .to_string(),
            ),
            "/agents" => SlashResult::Agents,
            "/agent" => {
                if arg.is_empty() {
                    SlashResult::Unknown(format!("Current agent: {}", self.current_agent))
                } else if let Some(msg) = self.select_agent_by_name(arg) {
                    SlashResult::Switched(msg)
                } else {
                    SlashResult::Unknown(format!("Unknown agent: {}", arg))
                }
            }
            "/default" => {
                if arg.is_empty() {
                    SlashResult::Unknown(format!("Default agent: {} (use /default <name> to change)", self.config.default_agent))
                } else if let Some(msg) = self.select_agent_by_name(arg) {
                    self.config.default_agent = self.current_agent.clone();
                    if let Err(e) = self.config.save() {
                        SlashResult::Unknown(format!("{}\nWarning: failed to save config: {}", msg, e))
                    } else {
                        SlashResult::Switched(format!("{}\nSaved as default agent.", msg))
                    }
                } else {
                    SlashResult::Unknown(format!("Unknown agent: {}", arg))
                }
            }
            "/tasks" => {
                let status_filter = if arg.is_empty() { None } else { Some(arg.to_string()) };
                SlashResult::Tasks { status_filter }
            }
            "/task" => {
                if arg.is_empty() {
                    SlashResult::Unknown("Usage: /task <title> [| description] [| priority]".into())
                } else {
                    // Parse: /task title | description | priority
                    let parts: Vec<&str> = arg.splitn(3, '|').collect();
                    let title = parts[0].trim().to_string();
                    let description = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
                    let priority = parts.get(2).map(|s| s.trim().to_string()).unwrap_or_else(|| "medium".into());
                    SlashResult::TaskCreate { title, description, priority }
                }
            }
            "/done" => {
                if arg.is_empty() {
                    SlashResult::Unknown("Usage: /done <task_id>".into())
                } else {
                    SlashResult::TaskUpdate { task_id: arg.to_string(), status: "done".into() }
                }
            }
            "/schedules" => SlashResult::Schedules,
            "/credits" => SlashResult::Credits,
            "/history" => {
                let limit = arg.parse::<u32>().unwrap_or(10);
                SlashResult::History { limit }
            }
            "/memories" | "/memory" => {
                let limit = arg.parse::<u32>().unwrap_or(20);
                SlashResult::Memories { limit }
            }
            "/provision" | "/sync" => SlashResult::Provision,
            "/connect" => SlashResult::Connect,
            "/reset" => SlashResult::Reset,
            "/clear" => SlashResult::Clear,
            "/quit" | "/q" | "/exit" => SlashResult::Quit,
            _ => SlashResult::Unknown(format!("Unknown command: {}", command)),
        }
    }

    pub fn finish_provision(&mut self, remote_agents: &[serde_json::Value]) -> Vec<String> {
        self.agents = db::parse_agents(remote_agents);
        let synced: Vec<String> = self.agents.iter()
            .map(|a| format!("{} ({})", a.capability, a.name))
            .collect();
        self.resolve_current_agent();
        synced
    }
}
