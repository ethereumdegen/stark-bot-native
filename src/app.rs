use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::db::Database;
use crate::event::AppEvent;
use crate::starflask::{self, ProgressEvent, StarflaskClient};
use crate::theme::Theme;

/// Input mode (vi-like).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
}

/// Which screen is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Chat,
    Agents,
    Help,
}

/// A single chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,     // "user", "agent", "system", "progress"
    pub content: String,
}

pub struct App {
    pub running: bool,
    pub mode: Mode,
    pub screen: Screen,
    pub theme: Theme,
    pub config: Config,
    pub db: Arc<Database>,
    pub client: Option<StarflaskClient>,

    // Chat state
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    pub scroll_offset: usize,
    pub auto_scroll: bool,

    // Agent state
    pub current_agent: String,
    pub current_agent_id: Option<String>,
    pub agents: Vec<crate::db::Agent>,
    pub agent_selection: usize,

    // Query state
    pub querying: bool,

    // Event channel for async tasks to send back
    pub event_tx: Option<mpsc::UnboundedSender<AppEvent>>,
}

impl App {
    pub fn new(config: Config, db: Database) -> Self {
        let client = StarflaskClient::new(&config).ok();
        let current_agent = config.default_agent.clone();

        Self {
            running: true,
            mode: Mode::Normal,
            screen: Screen::Chat,
            theme: Theme::default(),
            config,
            db: Arc::new(db),
            client,

            messages: vec![ChatMessage {
                role: "system".into(),
                content: "Welcome to starkbot. Press 'i' to type, '?' for help.".into(),
            }],
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            auto_scroll: true,

            current_agent: current_agent,
            current_agent_id: None,
            agents: Vec::new(),
            agent_selection: 0,

            querying: false,
            event_tx: None,
        }
    }

    pub fn set_event_tx(&mut self, tx: mpsc::UnboundedSender<AppEvent>) {
        self.event_tx = Some(tx);
    }

    /// Load agents from DB and resolve current agent ID.
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

    pub fn select_agent(&mut self, idx: usize) {
        if idx < self.agents.len() {
            self.agent_selection = idx;
            let agent = &self.agents[idx];
            self.current_agent = agent.capability.clone();
            self.current_agent_id = Some(agent.agent_id.clone());
            self.push_system(&format!("Switched to agent: {}", agent.name));
            self.screen = Screen::Chat;
        }
    }

    pub fn push_message(&mut self, role: &str, content: &str) {
        self.messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
        });
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn push_system(&mut self, content: &str) {
        self.push_message("system", content);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.messages.len().saturating_sub(1);
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
        self.push_message("progress", "Sending query...");

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
                // Update the last progress message
                if let Some(msg) = self.messages.last_mut() {
                    if msg.role == "progress" {
                        msg.content = format!("Status: {}", status);
                        return;
                    }
                }
                self.push_message("progress", &format!("Status: {}", status));
            }
            ProgressEvent::LogEntry { summary, .. } => {
                // Update the last progress message
                if let Some(msg) = self.messages.last_mut() {
                    if msg.role == "progress" {
                        msg.content = summary;
                        return;
                    }
                }
                self.push_message("progress", &summary);
            }
            ProgressEvent::Error(e) => {
                self.push_message("system", &format!("Warning: {}", e));
            }
        }
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn handle_query_complete(&mut self, result: Result<String, String>) {
        self.querying = false;
        // Remove last progress message
        if let Some(msg) = self.messages.last() {
            if msg.role == "progress" {
                self.messages.pop();
            }
        }

        match result {
            Ok(text) => {
                if text.is_empty() {
                    self.push_message("agent", "(empty response)");
                } else {
                    self.push_message("agent", &text);
                }
                let _ = self.db.log_message(
                    &self.current_agent, None,
                    self.messages.last().map(|m| m.content.as_str()).unwrap_or(""),
                    "agent",
                );
            }
            Err(e) => {
                self.push_message("error", &format!("Error: {}", e));
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
