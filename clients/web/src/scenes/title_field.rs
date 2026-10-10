use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::inputs::typed;
use crate::scenes::scene_page_state::ScenePageState;

pub fn title_field(state: ScenePageState) -> impl IntoView {
    html::section().class("about").child(
        html::input()
            .r#type("text")
            .class("scene-title")
            .attr("aria-label", "Scene title")
            .attr(
                "placeholder",
                "Untitled \u{2014} the outline shows its opening words",
            )
            .prop("value", move || state.title())
            .on(ev::change, move |event| {
                if let Some(field) = typed(&event) {
                    state.retitle(field.value());
                }
            }),
    )
}
