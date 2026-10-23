use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};

use crate::accounts::listed::ListedAccount;
use crate::accounts::service;
use crate::dialog;
use crate::http::ApiError;
use crate::problem;
use crate::speech;

fn naming(key: &'static str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name.to_owned());

    speech::say_with(key, &args)
}

pub fn role_dialog(
    account: ListedAccount,
    yourself: bool,
    changed: impl Fn() + Clone + 'static,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);
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
    let id = account.id;

    let change = move || {
        if asking.get_untracked() {
            return;
        }
        asking.set(true);
        refused.set(None);
        let (id, changed) = (id.clone(), changed.clone());
        task::spawn_local(async move {
            let answer = if dismissing {
                service::dismiss_operator(&id).await
            } else {
                service::appoint_operator(&id).await
            };
            match answer {
                Ok(()) => {
                    changed();
                    close();
                }
                Err(failed) => refused.set(Some(failed)),
            }
            asking.set(false);
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
                    .disabled(move || asking.get())
                    .on(ev::click, move |_| change())
                    .child(speech::said(confirm_key)),
            )),
        ),
    )
}
