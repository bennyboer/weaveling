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

pub fn role_dialog<Changed>(
    account: ListedAccount,
    yourself: bool,
    change: impl Fn() -> Changed + 'static,
    changing: impl Fn() -> bool + Send + Sync + 'static,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView
where
    Changed: Future<Output = bool> + 'static,
{
    let dismissing = account.operator;
    let (title_key, hint_key, confirm_key) = match (dismissing, yourself) {
        (false, _) => (
            "instance-appoint-dialog",
            "instance-appoint-hint",
            "instance-appoint-confirm",
        ),
        (true, false) => (
            "instance-dismiss-dialog",
            "instance-dismiss-hint",
            "instance-dismiss-confirm",
        ),
        (true, true) => (
            "instance-dismiss-dialog",
            "instance-dismiss-yourself-hint",
            "instance-dismiss-confirm",
        ),
    };
    let name = account.name.clone();
    let title = Signal::derive(move || naming(title_key, &name));

    let start = move || {
        let changed = change();
        task::spawn_local(async move {
            if changed.await {
                close();
            }
        });
    };

    dialog::modal(
        title,
        "change-role",
        close,
        (
            html::p()
                .class("hint")
                .child(naming(hint_key, &account.name)),
            move || refused.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("instance-role-cancel")),
                html::button()
                    .r#type("button")
                    .class(if dismissing { "danger" } else { "primary" })
                    .attr("aria-busy", move || changing().then_some("true"))
                    .on(ev::click, move |_| start())
                    .child(speech::said(confirm_key)),
            )),
        ),
    )
}
