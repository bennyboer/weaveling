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

/// Asks the operator to be sure before turning someone's second factor off, and says what follows.
pub fn second_factor_dialog<Removed>(
    account: ListedAccount,
    remove: impl Fn() -> Removed + 'static,
    removing: impl Fn() -> bool + Send + Sync + 'static,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView
where
    Removed: Future<Output = bool> + 'static,
{
    let name = account.name.clone();
    let title = Signal::derive(move || naming("instance-two-step-dialog", &name));

    let start = move || {
        let removed = remove();
        task::spawn_local(async move {
            if removed.await {
                close();
            }
        });
    };

    dialog::modal(
        title,
        "turn-off-two-step",
        close,
        (
            html::p()
                .class("hint")
                .child(naming("instance-two-step-hint", &account.name)),
            html::p()
                .class("hint")
                .child(speech::said("instance-two-step-recorded")),
            move || refused.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("instance-two-step-cancel")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || removing().then_some("true"))
                    .on(ev::click, move |_| start())
                    .child(speech::said("instance-two-step-confirm")),
            )),
        ),
    )
}
