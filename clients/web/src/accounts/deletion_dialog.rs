use std::future::Future;

use accounts_contract::refused;
use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};
use time::OffsetDateTime;

use crate::accounts::listed::ListedAccount;
use crate::dialog;
use crate::http::ApiError;
use crate::moment;
use crate::problem;
use crate::speech;

fn naming(key: &'static str, args: &[(&'static str, String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, value.clone());
    }

    speech::say_with(key, &told)
}

/// Asks the operator to confirm before another account is marked for deletion.
/// The dialog says when the account will be deleted.
pub fn deletion_dialog<Marked>(
    account: ListedAccount,
    until: OffsetDateTime,
    mark: impl Fn() -> Marked + 'static,
    marking: impl Fn() -> bool + Send + Sync + 'static,
    refused: RwSignal<Option<ApiError>>,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView
where
    Marked: Future<Output = bool> + 'static,
{
    let name = account.name.clone();
    let title = Signal::derive(move || naming("instance-delete-dialog", &[("name", name.clone())]));
    let told = naming(
        "instance-delete-hint",
        &[
            ("name", account.name.clone()),
            ("until", moment::human_day(until)),
        ],
    );
    let named = account.name.clone();
    let start = move || {
        let marked = mark();
        task::spawn_local(async move {
            if marked.await {
                close();
            }
        });
    };

    dialog::modal(
        title,
        "deleting-account",
        close,
        (
            html::p().class("hint").child(told),
            move || {
                refused.get().map(|failed| match &failed {
                    ApiError::Rejected(refusal)
                        if refusal.kind.as_deref() == Some(refused::PROJECTS_TO_HAND_OVER) =>
                    {
                        html::p()
                            .class("problem")
                            .role("alert")
                            .child(naming(
                                "instance-delete-hand-over",
                                &[("name", named.clone())],
                            ))
                            .into_any()
                    }
                    _ => problem::problem(failed).into_any(),
                })
            },
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("instance-delete-cancel")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || marking().then_some("true"))
                    .on(ev::click, move |_| start())
                    .child(speech::said("instance-delete-confirm")),
            )),
        ),
    )
}
