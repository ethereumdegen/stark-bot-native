use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    if app.agents.is_empty() {
        let msg = Paragraph::new("No agents loaded. Run `starkbot provision` to sync agents.")
            .block(Block::default().borders(Borders::ALL).title(" agents "));
        frame.render_widget(msg, area);
        return;
    }

    let lines: Vec<Line> = app.agents.iter().enumerate().map(|(i, agent)| {
        let marker = if i == app.agent_selection { "> " } else { "  " };
        let style = if i == app.agent_selection { theme.table_selected } else { theme.system_msg };
        Line::from(vec![
            Span::styled(marker, style),
            Span::styled(&agent.capability, theme.agent_name),
            Span::styled(format!("  {}", agent.name), style),
            Span::styled(format!("  [{}]", agent.status), theme.system_msg),
        ])
    }).collect();

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" agents (Enter to select, Esc to go back) "));

    frame.render_widget(paragraph, area);
}

/// Print agents as a table to stdout (for CLI mode).
pub fn print_agents_table(agents: &[crate::db::Agent]) {
    if agents.is_empty() {
        println!("No agents found. Run `starkbot provision` to sync agents.");
        return;
    }

    println!("{:<16} {:<36} {:<24} {}", "CAPABILITY", "AGENT_ID", "NAME", "STATUS");
    println!("{}", "-".repeat(80));
    for agent in agents {
        println!("{:<16} {:<36} {:<24} {}", agent.capability, agent.agent_id, agent.name, agent.status);
    }
}
