use iocraft::prelude::*;

use super::theme;

#[derive(Default, Props)]
pub struct InputBarProps<'a> {
    pub prompt: &'a str,
    pub children: Vec<AnyElement<'a>>,
}

#[component]
pub fn InputBar<'a>(props: &mut InputBarProps<'a>) -> impl Into<AnyElement<'a>> {
    let prompt = if props.prompt.is_empty() {
        "you> "
    } else {
        props.prompt
    };

    element! {
        View(
            width: 100pct,
            border_style: BorderStyle::Single,
            border_color: theme::COLOR_DIM,
            border_edges: Edges::Top,
            padding_left: 1,
            padding_right: 1,
        ) {
            Text(content: prompt, color: theme::COLOR_CYAN, weight: Weight::Bold)
            #(props.children.drain(..))
        }
    }
}
