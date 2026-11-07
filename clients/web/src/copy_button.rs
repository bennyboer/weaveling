use std::time::Duration;

use leptos::prelude::*;
use leptos::{IntoView, ev, html, task};
use wasm_bindgen_futures::JsFuture;

use crate::speech;

const TOLD_FOR: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Copying {
    Waiting,
    Copied,
    Refused,
}

/// A button that copies a text and says for a moment whether the browser let it, as wide all along as the longest of its words.
pub fn copy_button(label: &'static str, text: String) -> impl IntoView {
    let copying = RwSignal::new(Copying::Waiting);
    let said = speech::said(label);
    let copied = speech::said("copy-done");
    let refused = speech::said("copy-refused");

    html::button()
        .r#type("button")
        .class("copying")
        .class(("copied", move || copying.get() == Copying::Copied))
        .on(ev::click, move |_| {
            let text = text.clone();
            task::spawn_local(async move {
                let written = window().navigator().clipboard().write_text(&text);
                copying.set(match JsFuture::from(written).await {
                    Ok(_) => Copying::Copied,
                    Err(_) => Copying::Refused,
                });
                _ = set_timeout(move || copying.set(Copying::Waiting), TOLD_FOR);
            });
        })
        .child((
            html::span()
                .role("status")
                .child(move || match copying.get() {
                    Copying::Waiting => said(),
                    Copying::Copied => copied(),
                    Copying::Refused => refused(),
                }),
            html::span()
                .class("sizing")
                .attr("aria-hidden", "true")
                .child(said),
            html::span()
                .class("sizing")
                .attr("aria-hidden", "true")
                .child(copied),
            html::span()
                .class("sizing")
                .attr("aria-hidden", "true")
                .child(refused),
        ))
}
