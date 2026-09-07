use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

pub fn laid_out(
    work: AnyView,
    kept: AnyView,
    waiting: impl Fn() -> usize + Send + Sync + 'static,
) -> impl IntoView {
    let out = RwSignal::new(false);

    html::div().class("laid-out").child((
        work,
        html::button()
            .r#type("button")
            .class("tray-toggle")
            .attr("aria-expanded", move || out.get().to_string())
            .on(ev::click, move |_| out.update(|shown| *shown = !*shown))
            .child(move || format!("Pieces \u{00b7} {}", waiting())),
        html::aside()
            .class(move || match out.get() {
                true => "tray pulled-out",
                false => "tray",
            })
            .child((
                html::button()
                    .r#type("button")
                    .class("shut")
                    .attr("aria-label", "Hide the pieces")
                    .on(ev::click, move |_| out.set(false))
                    .child("\u{00d7}"),
                kept,
            )),
    ))
}
