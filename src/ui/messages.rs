use iocraft::prelude::*;

use crate::app::ChatMessage;
use super::message::{UserMessage, AgentMessage, SystemMessage, ErrorMessage};

#[derive(Default, Props)]
pub struct MessageListProps {
    pub messages: Vec<ChatMessage>,
    pub current_agent: String,
}

#[component]
pub fn MessageList(props: &MessageListProps) -> impl Into<AnyElement<'static>> {
    element! {
        ScrollView {
            View(
                flex_direction: FlexDirection::Column,
                width: 100pct,
                padding_left: 1,
                padding_right: 1,
            ) {
                #(props.messages.iter().enumerate().map(|(i, msg)| {
                    let key = i as u64;
                    match msg.role.as_str() {
                        "user" => element! {
                            View(key, padding_top: if i > 0 { 1 } else { 0 }) {
                                UserMessage(content: msg.content.clone())
                            }
                        }.into_any(),
                        "agent" => element! {
                            View(key, padding_top: 1) {
                                AgentMessage(
                                    agent: props.current_agent.clone(),
                                    content: msg.content.clone(),
                                )
                            }
                        }.into_any(),
                        "system" => element! {
                            View(key) {
                                SystemMessage(content: msg.content.clone())
                            }
                        }.into_any(),
                        "error" => element! {
                            View(key) {
                                ErrorMessage(content: msg.content.clone())
                            }
                        }.into_any(),
                        _ => element! {
                            View(key) {
                                SystemMessage(content: msg.content.clone())
                            }
                        }.into_any(),
                    }
                }))
            }
        }
    }
}
