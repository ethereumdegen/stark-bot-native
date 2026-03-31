use iocraft::prelude::*;
use std::time::Duration;

use super::theme;

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Default, Props)]
pub struct SpinnerRowProps {
    pub text: String,
}

#[component]
pub fn SpinnerRow(props: &SpinnerRowProps, mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let mut frame = hooks.use_state(|| 0usize);

    hooks.use_future(async move {
        loop {
            smol::Timer::after(Duration::from_millis(80)).await;
            frame.set((frame.get() + 1) % FRAMES.len());
        }
    });

    element! {
        View(
            width: 100pct,
            padding_left: 1,
        ) {
            Text(content: FRAMES[frame.get()], color: theme::COLOR_YELLOW)
            Text(content: format!(" {}", props.text), color: theme::COLOR_DIM)
        }
    }
}
