use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::db::Database;
use crate::event::AppEvent;
use crate::starflask::{self, ProgressEvent, StarflaskClient};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Chat,
}

/// Result of a slash command.
pub enum SlashResult {
    /// Print help text.
    Help(String),
    /// Print agent list.
    Agents,
    /// Switched agent — message to display.
    Switched(String),
    /// Enter connect flow to set API key.
    Connect,
    /// Clear the screen.
    Clear,
    /// Quit the app.
    Quit,
    /// Unknown command.
    Unknown(String),
}

/// A single chat message (kept for DB logging).
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub config: Config,
    pub db: Arc<Database>,
    pub client: Option<StarflaskClient>,

    // Setup state
    pub setup_input: String,
    pub setup_cursor: usize,

    // Chat state
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,

    // Agent state
    pub current_agent: String,
    pub current_agent_id: Option<String>,
    pub agents: Vec<crate::db::Agent>,

    // Query state
    pub querying: bool,
    pub progress_text: Option<String>,

    // Event channel for async tasks
    pub event_tx: Option<mpsc::UnboundedSender<AppEvent>>,
}

impl App {
    pub fn new(config: Config, db: Database) -> Self {
        let client = StarflaskClient::new(&config).ok();
        let current_agent = config.default_agent.clone();

        Self {
            running: true,
            screen: if config.api_key().is_some() { Screen::Chat } else { Screen::Setup },
            config,
            db: Arc::new(db),
            client,

            setup_input: String::new(),
            setup_cursor: 0,

            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,

            current_agent,
            current_agent_id: None,
            agents: Vec::new(),

            querying: false,
            progress_text: None,
            event_tx: None,
        }
    }

    pub fn set_event_tx(&mut self, tx: mpsc::UnboundedSender<AppEvent>) {
        self.event_tx = Some(tx);
    }

    /// Called after setup saves an API key.
    pub fn finish_setup(&mut self) {
        self.client = StarflaskClient::new(&self.config).ok();
        self.load_agents();
        self.screen = Screen::Chat;
    }

    pub fn load_agents(&mut self) {
        if let Ok(agents) = self.db.list_agents() {
            self.agents = agents;
        }
        self.resolve_current_agent();
    }

    fn resolve_current_agent(&mut self) {
        self.current_agent_id = self.agents.iter()
            .find(|a| a.capability == self.current_agent)
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
            self.push_message("system", &msg);
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
                "/help         Show this help\r\n\
                 /agents       List available agents\r\n\
                 /agent <n>    Switch to agent by name\r\n\
                 /connect      Set API key\r\n\
                 /clear        Clear the screen\r\n\
                 /quit         Exit starkbot"
                    .to_string(),
            ),
            "/agents" => SlashResult::Agents,
            "/agent" => {
                if arg.is_empty() {
                    SlashResult::Unknown("Usage: /agent <name>".to_string())
                } else if let Some(msg) = self.select_agent_by_name(arg) {
                    SlashResult::Switched(msg)
                } else {
                    SlashResult::Unknown(format!("Unknown agent: {}", arg))
                }
            }
            "/connect" => SlashResult::Connect,
            "/clear" => SlashResult::Clear,
            "/quit" | "/q" | "/exit" => SlashResult::Quit,
            _ => SlashResult::Unknown(format!("Unknown command: {}", command)),
        }
    }

    pub fn push_message(&mut self, role: &str, content: &str) {
        self.messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
        });
    }

    /// Submit the current input as a query.
    pub fn submit_query(&mut self) {
        let message = self.input.trim().to_string();
        if message.is_empty() { return; }

        self.input.clear();
        self.cursor_pos = 0;
        self.push_message("user", &message);

        let Some(agent_id) = self.current_agent_id.clone() else {
            self.push_message("error", "No agent selected. Run `starkbot provision` first.");
            return;
        };

        let Some(ref _client) = self.client else {
            self.push_message("error", "Not connected. Check STARFLASK_API_KEY.");
            return;
        };

        self.querying = true;
        self.progress_text = Some("Sending query...".to_string());

        // Log to DB
        let capability = self.current_agent.clone();
        let _ = self.db.log_message(&capability, None, &message, "user");

        // Spawn async polling task
        let tx = self.event_tx.clone().unwrap();
        let client = StarflaskClient::new(&self.config);
        let msg = message.clone();

        tokio::spawn(async move {
            let client = match client {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(AppEvent::QueryComplete(Err(e)));
                    return;
                }
            };

            let session_id = match client.create_session(&agent_id, &msg).await {
                Ok(id) => id,
                Err(e) => {
                    let _ = tx.send(AppEvent::QueryComplete(Err(e)));
                    return;
                }
            };

            let tx_progress = tx.clone();
            let result = client.poll_session(&agent_id, &session_id, move |evt| {
                let _ = tx_progress.send(AppEvent::Progress(evt));
            }).await;

            match result {
                Ok(session) => {
                    let text = starflask::parse_text_result(&session.result);
                    let _ = tx.send(AppEvent::QueryComplete(Ok(text)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::QueryComplete(Err(e)));
                }
            }
        });
    }

    pub fn handle_progress(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::StatusChange(status) => {
                self.progress_text = Some(format!("Status: {}", status));
            }
            ProgressEvent::LogEntry { summary, .. } => {
                self.progress_text = Some(summary);
            }
            ProgressEvent::Error(e) => {
                self.progress_text = Some(format!("Warning: {}", e));
            }
        }
    }

    pub fn handle_query_complete(&mut self, result: Result<String, String>) -> String {
        self.querying = false;
        self.progress_text = None;

        match result {
            Ok(text) => {
                let content = if text.is_empty() { "(empty response)".to_string() } else { text };
                self.push_message("agent", &content);
                let _ = self.db.log_message(&self.current_agent, None, &content, "agent");
                content
            }
            Err(e) => {
                let content = format!("Error: {}", e);
                self.push_message("error", &content);
                content
            }
        }
    }

    // ── Input editing ──

    pub fn input_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn input_backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
            self.input.remove(self.cursor_pos);
        }
    }

    pub fn input_delete(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
        }
    }

    pub fn input_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
        }
    }

    pub fn input_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            let next = self.input[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos += next;
        }
    }

    pub fn input_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn input_end(&mut self) {
        self.cursor_pos = self.input.len();
    }
}
