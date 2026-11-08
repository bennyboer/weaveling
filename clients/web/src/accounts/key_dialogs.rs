use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::act::Act;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::speech;

fn naming(key: &'static str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name.to_owned());

    speech::say_with(key, &args)
}

fn name_field(name: RwSignal<String>) -> impl IntoView {
    html::label().class("field").child((
        html::span().child(speech::said("key-name")),
        html::input()
            .r#type("text")
            .attr("maxlength", "60")
            .attr("autocomplete", "off")
            .attr("placeholder", speech::said("key-name-example"))
            .required(true)
            .prop("value", move || name.get())
            .on(ev::input, move |event| name.set(event_target_value(&event))),
    ))
}

/// Names the key the browser has just made, which is added only once it is named.
pub fn naming_the_new_key(
    done: Act<String>,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let name = RwSignal::new(String::new());

    dialog::modal(
        Signal::derive(|| speech::say("key-made-heading")),
        "naming-key",
        close,
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                done.start(name.get_untracked());
            })
            .child((
                html::p().class("hint").child(speech::said("key-made-hint")),
                name_field(name),
                move || refused.get().map(problem::problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| close())
                        .child(speech::said("key-made-dismiss")),
                    html::button()
                        .r#type("submit")
                        .class("primary")
                        .attr("aria-busy", move || done.busy())
                        .child(speech::said("key-made-done")),
                )),
            )),
    )
}

pub fn renaming(
    was: String,
    done: Act<String>,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let title = naming("key-rename-heading", &was);
    let name = RwSignal::new(was);

    dialog::modal(
        Signal::derive(move || title.clone()),
        "renaming-key",
        close,
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                done.start(name.get_untracked());
            })
            .child((
                name_field(name),
                move || refused.get().map(problem::problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| close())
                        .child(speech::said("two-step-cancel")),
                    html::button()
                        .r#type("submit")
                        .class("primary")
                        .attr("aria-busy", move || done.busy())
                        .child(speech::said("key-rename-confirm")),
                )),
            )),
    )
}

/// Asks for the password before a key is removed, saying what follows.
pub fn removing(
    named: &str,
    done: Act<String>,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let title = naming("key-remove-heading", named);
    let password = RwSignal::new(String::new());

    dialog::modal(
        Signal::derive(move || title.clone()),
        "removing-key",
        close,
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                done.start(password.get_untracked());
            })
            .child((
                html::p()
                    .class("hint")
                    .child(speech::said("key-remove-hint")),
                html::label().class("field").child((
                    html::span().child(speech::said("key-remove-password")),
                    html::input()
                        .r#type("password")
                        .attr("autocomplete", "current-password")
                        .required(true)
                        .prop("value", move || password.get())
                        .on(ev::input, move |event| {
                            password.set(event_target_value(&event))
                        }),
                )),
                move || refused.get().map(problem::problem),
                html::div().class("dialog-actions").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| close())
                        .child(speech::said("two-step-cancel")),
                    html::button()
                        .r#type("submit")
                        .class("danger")
                        .attr("aria-busy", move || done.busy())
                        .child(speech::said("key-remove-confirm")),
                )),
            )),
    )
}
