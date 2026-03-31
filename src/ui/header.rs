use iocraft::prelude::*;

use super::theme;

#[derive(Default, Props)]
pub struct HeaderBarProps {
    pub agent: String,
    pub connected: bool,
    pub project: Option<String>,
    pub credits: Option<i64>,
}

#[component]
pub fn HeaderBar(props: &HeaderBarProps) -> impl Into<AnyElement<'static>> {
    let status = if props.connected { "connected" } else { "disconnected" };
    let status_color = if props.connected { theme::COLOR_GREEN } else { theme::COLOR_RED };

    let project_text = match &props.project {
        Some(pid) => format!(" | project: {}", &pid[..8.min(pid.len())]),
        None => String::new(),
    };

    let credits_text = match props.credits {
        Some(c) => format!(" | credits: {}", c),
        None => String::new(),
    };

    element! {
        View(
            width: 100pct,
            border_style: BorderStyle::Single,
            border_color: theme::COLOR_HEADER_BORDER,
            border_edges: Edges::Bottom,
            padding_left: 1,
            padding_right: 1,
        ) {
            Text(
                content: format!("STARK-BOT v{}", env!("CARGO_PKG_VERSION")),
                color: theme::COLOR_WHITE,
                weight: Weight::Bold,
            )
            Text(content: format!(" | agent: {}", props.agent), color: theme::COLOR_CYAN)
            Text(content: project_text, color: theme::COLOR_DIM)
            Text(content: credits_text, color: theme::COLOR_YELLOW)
            Text(content: " | ", color: theme::COLOR_DIM)
            Text(content: status, color: status_color)
        }
    }
}
