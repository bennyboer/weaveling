use leptos::prelude::*;
use leptos::{IntoView, ev, html};

pub fn modal(
    title: &'static str,
    kind: &'static str,
    close: impl Fn() + Copy + Send + Sync + 'static,
    body: impl IntoView + 'static,
) -> impl IntoView {
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            close();
        }
    });
    on_cleanup(move || escaping.remove());

    html::div()
        .class("overlay")
        .on(ev::click, move |_| close())
        .child(
            html::div()
                .class(format!("dialog {kind}").trim_end().to_owned())
                .role("dialog")
                .attr("aria-modal", "true")
                .attr("aria-label", title)
                .on(ev::click, |event| event.stop_propagation())
                .child((html::h2().child(title), body)),
        )
}
