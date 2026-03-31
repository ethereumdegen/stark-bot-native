use iocraft::prelude::*;

use super::theme;

#[derive(Default, Props)]
pub struct UserMessageProps {
    pub content: String,
}

#[component]
pub fn UserMessage(props: &UserMessageProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(width: 100pct) {
            Text(content: "❯ ", color: theme::COLOR_CYAN, weight: Weight::Bold)
            Text(content: props.content.clone(), color: theme::COLOR_WHITE)
        }
    }
}

#[derive(Default, Props)]
pub struct AgentMessageProps {
    pub agent: String,
    pub content: String,
}

#[component]
pub fn AgentMessage(props: &AgentMessageProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(width: 100pct, flex_direction: FlexDirection::Column) {
            View {
                Text(
                    content: format!("{}> ", props.agent),
                    color: theme::COLOR_GREEN,
                    weight: Weight::Bold,
                )
            }
            View(padding_left: 2) {
                Text(content: props.content.clone())
            }
        }
    }
}

#[derive(Default, Props)]
pub struct SystemMessageProps {
    pub content: String,
}

#[component]
pub fn SystemMessage(props: &SystemMessageProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(width: 100pct) {
            Text(content: "--- ", color: theme::COLOR_DIM)
            Text(content: props.content.clone(), color: theme::COLOR_GRAY)
        }
    }
}

#[derive(Default, Props)]
pub struct ErrorMessageProps {
    pub content: String,
}

#[component]
pub fn ErrorMessage(props: &ErrorMessageProps) -> impl Into<AnyElement<'static>> {
    element! {
        View(width: 100pct) {
            Text(content: "✗ ", color: theme::COLOR_RED, weight: Weight::Bold)
            Text(content: props.content.clone(), color: theme::COLOR_RED)
        }
    }
}
