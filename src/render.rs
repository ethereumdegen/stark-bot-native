use std::io::{self, Write};

use crate::theme::*;
use crate::db::Agent;

const SPLASH: &str = r#"
  _____ _____  _    ____  _  ______   ___ _____
 / ____|_   _|/ \  |  _ \| |/ /  _ \ / _ \_   _|
 \___ \  | | / _ \ | |_) | ' /| |_) | | | || |
  ___) | | |/ ___ \|  _ <| . \|  _ <| |_| || |
 |____/  |_/_/   \_\_| \_\_|\_\_| \_\\___/ |_|
"#;

pub struct InlineRenderer {
    out: io::Stdout,
}

/// Replace bare `\n` with `\r\n` for raw mode output.
fn raw_lines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

impl InlineRenderer {
    pub fn new() -> Self {
        Self { out: io::stdout() }
    }

    fn write_line(&mut self, text: &str) {
        let _ = write!(self.out, "{}\r\n", text);
        let _ = self.out.flush();
    }

    pub fn print_splash(&mut self, agent: &str, connected: bool) {
        let status = if connected { "connected" } else { "disconnected" };
        for line in SPLASH.lines() {
            let _ = write!(self.out, "{}{}{}\r\n", CYAN, line, RESET);
        }
        let _ = write!(self.out, " {}Starflask AI Agent Terminal{} v{}\r\n", BOLD, RESET, env!("CARGO_PKG_VERSION"));
        let _ = write!(self.out, " Agent: {}{}{} | Status: {}\r\n", YELLOW, agent, RESET, status);
        let _ = write!(self.out, " Type {}/help{} for commands\r\n", GRAY, RESET);
        self.write_line("");
        let _ = self.out.flush();
    }

    pub fn print_setup_prompt(&mut self) {
        let _ = write!(self.out, "{}No API key found.{}\r\n", YELLOW, RESET);
        let _ = write!(self.out, "Enter your STARFLASK_API_KEY (or press Esc to skip):\r\n");
        let _ = write!(self.out, "> ");
        let _ = self.out.flush();
    }

    pub fn print_user_message(&mut self, content: &str) {
        let _ = write!(self.out, "{}you>{} {}\r\n", CYAN, RESET, content);
        let _ = self.out.flush();
    }

    pub fn print_agent_message(&mut self, agent: &str, content: &str) {
        let content = raw_lines(content);
        let _ = write!(self.out, "{}{}>{} {}\r\n", GREEN, agent, RESET, content);
        let _ = self.out.flush();
    }

    pub fn print_system_message(&mut self, content: &str) {
        let content = raw_lines(content);
        let _ = write!(self.out, "{}--- {}{}\r\n", GRAY, content, RESET);
        let _ = self.out.flush();
    }

    pub fn print_error(&mut self, content: &str) {
        let content = raw_lines(content);
        let _ = write!(self.out, "{}ERR {}{}\r\n", RED, content, RESET);
        let _ = self.out.flush();
    }

    pub fn update_progress(&mut self, text: &str) {
        // Overwrite current line with progress
        let _ = write!(self.out, "\r\x1b[2K{}... {}{}", YELLOW, text, RESET);
        let _ = self.out.flush();
    }

    pub fn clear_progress(&mut self) {
        let _ = write!(self.out, "\r\x1b[2K");
        let _ = self.out.flush();
    }

    pub fn redraw_input(&mut self, prompt: &str, input: &str, cursor_pos: usize) {
        let cursor_col = prompt.len() + input[..cursor_pos].chars().count();
        let _ = write!(self.out, "\r\x1b[2K{}{}{}{}", CYAN, prompt, RESET, input);
        // Position cursor
        let _ = write!(self.out, "\r\x1b[{}C", cursor_col);
        let _ = self.out.flush();
    }

    pub fn print_agents(&mut self, agents: &[Agent], current: &str) {
        if agents.is_empty() {
            self.write_line(&format!("{}No agents provisioned. Run `starkbot provision` first.{}", YELLOW, RESET));
            return;
        }
        for a in agents {
            let marker = if a.capability == current { "[active] " } else { "" };
            let prefix = if a.capability == current { "  > " } else { "    " };
            let _ = write!(
                self.out,
                "{}{}{:<10}{} {:<20} {}{}{}\r\n",
                prefix, YELLOW, a.capability, RESET, a.name, GREEN, marker, RESET,
            );
        }
        let _ = self.out.flush();
    }

    pub fn print_help(&mut self, text: &str) {
        for line in text.lines() {
            let _ = write!(self.out, "  {}{}{}\r\n", GRAY, line, RESET);
        }
        let _ = self.out.flush();
    }

    pub fn clear_screen(&mut self) {
        let _ = write!(self.out, "\x1b[2J\x1b[H");
        let _ = self.out.flush();
    }

    pub fn newline(&mut self) {
        let _ = write!(self.out, "\r\n");
        let _ = self.out.flush();
    }
}
