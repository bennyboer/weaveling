use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::dialog::modal;
use crate::speech::said;
use crate::writing::write_view_state::WriteViewState;

pub fn settings_dialog(state: WriteViewState) -> impl IntoView {
    let settings = state.settings;

    move || {
        settings.is_open().then(|| {
            modal(
                Signal::derive(said("writing-settings")),
                "writing-settings",
                move || settings.close(),
                (
                    setting(
                        "setting-spelling",
                        "setting-spelling-hint",
                        move || settings.checks_spelling(),
                        move |wanted| settings.check_spelling(wanted),
                    ),
                    setting(
                        "setting-justified",
                        "setting-justified-hint",
                        move || settings.justifies(),
                        move |wanted| settings.justify(wanted),
                    ),
                    setting(
                        "setting-typewriter",
                        "setting-typewriter-hint",
                        move || settings.scrolls_like_a_typewriter(),
                        move |wanted| settings.scroll_like_a_typewriter(wanted),
                    ),
                    html::div().class("dialog-actions").child(
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| settings.close())
                            .child(said("settings-close")),
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
        html::span().child((said(named), html::small().child(said(told)))),
    ))
}
