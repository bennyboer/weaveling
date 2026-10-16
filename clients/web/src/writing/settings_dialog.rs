use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::inputs::typed;
use crate::writing::write_view_state::WriteViewState;

pub fn settings_dialog(state: WriteViewState) -> impl IntoView {
    let settings = state.settings;

    move || {
        settings.is_open().then(|| {
            let escaping = window_event_listener(ev::keydown, move |event| {
                if event.key() == "Escape" {
                    settings.close();
                }
            });
            on_cleanup(move || escaping.remove());

            html::div()
                .class("overlay")
                .on(ev::click, move |_| settings.close())
                .child(
                    html::div()
                        .class("dialog writing-settings")
                        .role("dialog")
                        .attr("aria-modal", "true")
                        .attr("aria-label", "Writing settings")
                        .on(ev::click, |event| event.stop_propagation())
                        .child((
                            html::h2().child("Writing settings"),
                            html::label().class("setting").child((
                                html::input()
                                    .r#type("checkbox")
                                    .prop("checked", move || settings.checks_spelling())
                                    .on(ev::change, move |event| {
                                        if let Some(field) = typed(&event) {
                                            settings.check_spelling(field.checked());
                                        }
                                    }),
                                html::span().child((
                                    "Check spelling",
                                    html::small().child(
                                        "Underlines words the browser's dictionary does not know.",
                                    ),
                                )),
                            )),
                            html::div().class("dialog-actions").child(
                                html::button()
                                    .r#type("button")
                                    .on(ev::click, move |_| settings.close())
                                    .child("Close"),
                            ),
                        )),
                )
        })
    }
}
