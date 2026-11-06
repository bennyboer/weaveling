use std::future::Future;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};

use crate::accounts::listed::ListedAccount;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::speech;

fn naming(key: &'static str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name.to_owned());

    speech::say_with(key, &args)
}

pub fn lock_dialog<Locked>(
    account: ListedAccount,
    lock: impl Fn(String) -> Locked + 'static,
    locking: impl Fn() -> bool + Send + Sync + 'static,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView
where
    Locked: Future<Output = bool> + 'static,
{
    let reason = RwSignal::new(String::new());
    let name = account.name.clone();
    let title = Signal::derive(move || naming("instance-lock-dialog", &name));

    let start = move || {
        let locked = lock(reason.get_untracked());
        task::spawn_local(async move {
            if locked.await {
                close();
            }
        });
    };

    dialog::modal(
        title,
        "lock-account",
        close,
        (
            html::p()
                .class("hint")
                .child(naming("instance-lock-hint", &account.name)),
            html::form()
                .on(ev::submit, move |event| {
                    event.prevent_default();
                    start();
                })
                .child((
                    html::label().class("field").child((
                        html::span().child(speech::said("instance-lock-reason")),
                        html::textarea()
                            .required(true)
                            .prop("value", move || reason.get())
                            .on(ev::input, move |event| {
                                reason.set(event_target_value(&event))
                            }),
                    )),
                    move || refused.get().map(problem::problem),
                    html::div().class("dialog-actions").child((
                        html::button()
                            .r#type("button")
                            .on(ev::click, move |_| close())
                            .child(speech::said("instance-lock-cancel")),
                        html::button()
                            .r#type("submit")
                            .class("danger")
                            .attr("aria-busy", move || locking().then_some("true"))
                            .child(speech::said("instance-lock-confirm")),
                    )),
                )),
        ),
    )
}
