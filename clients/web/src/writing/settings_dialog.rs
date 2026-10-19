use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::dialog::modal;
use crate::writing::write_view_state::WriteViewState;

const SPELLING: &str = "Underlines words the browser's dictionary does not know.";
const JUSTIFIED: &str =
    "Sets lines flush on both sides like a printed page, hyphenating words in the book's language.";
const TYPEWRITER: &str = "Keeps the line you are writing in the middle of the window.";

pub fn settings_dialog(state: WriteViewState) -> impl IntoView {
    let settings = state.settings;

    move || {
        settings.is_open().then(|| {
            modal(
                "Writing settings",
                "writing-settings",
                move || settings.close(),
                (
                    setting(
                        "Check spelling",
                        SPELLING,
                        move || settings.checks_spelling(),
                        move |wanted| settings.check_spelling(wanted),
                    ),
                    setting(
                        "Justified text",
                        JUSTIFIED,
                        move || settings.justifies(),
                        move |wanted| settings.justify(wanted),
                    ),
                    setting(
                        "Typewriter scrolling",
                        TYPEWRITER,
                        move || settings.scrolls_like_a_typewriter(),
                        move |wanted| settings.scroll_like_a_typewriter(wanted),
                    ),
                    html::div().class("dialog-actions").child(
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| settings.close())
                            .child("Close"),
                    ),
                ),
            )
        })
    }
}

fn setting(
    named: &'static str,
    told: &'static str,
    on: impl Fn() -> bool + Send + Sync + 'static,
    turned: impl Fn(bool) + 'static,
) -> impl IntoView {
    html::label().class("setting").child((
        html::input()
            .r#type("checkbox")
            .prop("checked", on)
            .on(ev::change, move |event| {
                turned(event_target_checked(&event));
            }),
        html::span().child((named, html::small().child(told))),
    ))
}
