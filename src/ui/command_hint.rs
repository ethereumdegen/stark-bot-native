use iocraft::prelude::*;

use super::theme;

pub struct SlashCmd {
    pub name: &'static str,
    pub args: &'static str,
    pub desc: &'static str,
}

pub const COMMANDS: &[SlashCmd] = &[
    SlashCmd { name: "/help",      args: "",              desc: "Show this help" },
    SlashCmd { name: "/agents",    args: "",              desc: "List available agents" },
    SlashCmd { name: "/agent",     args: "<name>",        desc: "Switch to agent" },
    SlashCmd { name: "/default",   args: "<name>",        desc: "Set default agent" },
    SlashCmd { name: "/tasks",     args: "[status]",      desc: "List project tasks" },
    SlashCmd { name: "/task",      args: "<title>",       desc: "Create a new task" },
    SlashCmd { name: "/done",      args: "<task_id>",     desc: "Mark task as done" },
    SlashCmd { name: "/schedules", args: "",              desc: "List agent schedules" },
    SlashCmd { name: "/credits",   args: "",              desc: "Show credit balance" },
    SlashCmd { name: "/history",   args: "[n]",           desc: "Recent sessions" },
    SlashCmd { name: "/memories",  args: "[n]",           desc: "Show agent memories" },
    SlashCmd { name: "/provision", args: "",              desc: "Sync agents from API" },
    SlashCmd { name: "/connect",   args: "",              desc: "Set API key" },
    SlashCmd { name: "/reset",     args: "",              desc: "Wipe config & start fresh" },
    SlashCmd { name: "/clear",     args: "",              desc: "Clear the screen" },
    SlashCmd { name: "/quit",      args: "",              desc: "Exit stark-bot" },
];

/// Filter commands by prefix match. Returns empty if input contains a space
/// (user is typing args) or doesn't start with `/`.
pub fn filter_commands(input: &str) -> Vec<&'static SlashCmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return vec![];
    }
    let query = input.to_lowercase();
    COMMANDS.iter().filter(|c| c.name.starts_with(&query)).collect()
}

#[derive(Default, Props)]
pub struct CommandHintProps {
    pub commands: Vec<(&'static str, &'static str, &'static str)>,
    pub selected: usize,
    pub width: u32,
}

#[component]
pub fn CommandHint(props: &CommandHintProps) -> impl Into<AnyElement<'static>> {
    let rows: Vec<AnyElement> = props
        .commands
        .iter()
        .enumerate()
        .map(|(i, (name, args, desc))| {
            let is_selected = i == props.selected;
            let prefix = if is_selected { " > " } else { "   " };
            let name_color = if is_selected { theme::COLOR_CYAN } else { theme::COLOR_GRAY };
            let name_weight = if is_selected { Weight::Bold } else { Weight::Normal };
            let desc_color = if is_selected { theme::COLOR_WHITE } else { theme::COLOR_DIM };

            let cmd_text = if args.is_empty() {
                format!("{}{}", prefix, name)
            } else {
                format!("{}{} {}", prefix, name, args)
            };

            element! {
                View(width: 100pct, flex_direction: FlexDirection::Row) {
                    Text(
                        content: format!("{:<22}", cmd_text),
                        color: name_color,
                        weight: name_weight,
                    )
                    Text(
                        content: desc.to_string(),
                        color: desc_color,
                    )
                }
            }
            .into_any()
        })
        .collect();

    element! {
        View(
            width: 100pct,
            border_style: BorderStyle::Single,
            border_color: theme::COLOR_DIM,
            padding_left: 1,
            padding_right: 1,
        ) {
            #(rows)
        }
    }
}
