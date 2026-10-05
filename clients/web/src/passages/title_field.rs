use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::inputs::typed;
use crate::passages::open_passage::OpenPassage;

pub fn title_field(open: OpenPassage) -> impl IntoView {
    html::section().class("about").child(
        html::input()
            .r#type("text")
            .class("passage-title")
            .attr("aria-label", "Passage title")
            .attr(
                "placeholder",
                "Untitled \u{2014} the outline shows its opening words",
            )
            .prop("value", move || open.title())
            .on(ev::change, move |event| {
                if let Some(field) = typed(&event) {
                    open.retitle(field.value());
                }
            }),
    )
}
