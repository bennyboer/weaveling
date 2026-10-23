use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::accounts::listed::ListedAccount;
use crate::accounts::lock_dialog;
use crate::accounts::roster::Roster;
use crate::moment;
use crate::problem;
use crate::speech;

fn said_with(key: &'static str, args: &[(&'static str, String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, value.clone());
    }

    speech::say_with(key, &told)
}

pub fn accounts_listing() -> impl IntoView {
    let roster = Roster::asked();

    (
        html::h2().child(speech::said("instance-accounts-heading")),
        move || roster.refused.get().map(problem::problem),
        html::ul().class("accounts").child(move || {
            roster
                .accounts
                .get()
                .into_iter()
                .map(|account| listed(roster, account))
                .collect_view()
        }),
        move || {
            roster.next.get().map(|next| {
                html::button()
                    .r#type("button")
                    .class("more")
                    .disabled(move || roster.loading.get())
                    .on(ev::click, move |_| roster.load(Some(next.clone())))
                    .child(speech::said("instance-accounts-more"))
            })
        },
        move || {
            roster.locking.get().map(|account| {
                let id = account.id.clone();
                lock_dialog::lock_dialog(
                    account,
                    move |lock| roster.mark(&id, Some(lock)),
                    move || roster.locking.set(None),
                )
            })
        },
    )
}

fn listed(roster: Roster, account: ListedAccount) -> impl IntoView {
    let signed_up = said_with(
        "instance-account-signed-up",
        &[("when", moment::human_time(account.signed_up_at))],
    );
    let action = (!account.operator).then(|| {
        let id = account.id.clone();
        if account.lock.is_some() {
            html::button()
                .r#type("button")
                .on(ev::click, move |_| roster.unlock(id.clone()))
                .child(speech::said("instance-account-unlock"))
                .into_any()
        } else {
            let chosen = account.clone();
            html::button()
                .r#type("button")
                .class("danger")
                .on(ev::click, move |_| roster.locking.set(Some(chosen.clone())))
                .child(speech::said("instance-account-lock"))
                .into_any()
        }
    });

    html::li().child((
        html::div().class("who").child((
            html::p().class("name").child(account.name.clone()),
            html::p().class("email").child(account.email.clone()),
            html::p().class("signed-up").child(signed_up),
        )),
        html::div().class("marks").child((
            (!account.verified).then(|| {
                html::span()
                    .class("mark")
                    .child(speech::said("instance-account-unverified"))
            }),
            account.operator.then(|| {
                html::span()
                    .class("mark")
                    .child(speech::said("instance-account-operator"))
            }),
            account.lock.map(|lock| {
                html::p().class("lock").child((
                    html::span().class("mark locked").child(said_with(
                        "instance-account-locked",
                        &[("when", moment::human_time(lock.since))],
                    )),
                    html::span().class("reason").child(lock.reason),
                ))
            }),
        )),
        action,
    ))
}
