use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};
use time::OffsetDateTime;

use crate::accounts::listed::{ListedAccount, OperatorLock};
use crate::accounts::service;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::speech;

fn now() -> OffsetDateTime {
    let millis = js_sys::Date::now() as i64;

    OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

fn naming(key: &'static str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name.to_owned());

    speech::say_with(key, &args)
}

pub fn lock_dialog(
    account: ListedAccount,
    locked: impl Fn(OperatorLock) + Clone + 'static,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let reason = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);
    let name = account.name.clone();
    let title = Signal::derive(move || naming("instance-lock-dialog", &name));
    let id = account.id;

    let lock = move || {
        if asking.get_untracked() {
            return;
        }
        asking.set(true);
        refused.set(None);
        let (id, typed, locked) = (id.clone(), reason.get_untracked(), locked.clone());
        task::spawn_local(async move {
            match service::lock_account(&id, &typed).await {
                Ok(()) => {
                    locked(OperatorLock {
                        reason: typed.trim().to_owned(),
                        since: now(),
                    });
                    close();
                }
                Err(failed) => refused.set(Some(failed)),
            }
            asking.set(false);
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
                    lock();
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
                            .disabled(move || asking.get())
                            .child(speech::said("instance-lock-confirm")),
                    )),
                )),
        ),
    )
}
