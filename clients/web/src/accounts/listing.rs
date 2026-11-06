use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::accounts::listed::ListedAccount;
use crate::accounts::roster;
use crate::accounts::roster::{Intent, Roster};
use crate::accounts::{CurrentSession, SignInState, lock_dialog, role_dialog};
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
    let session = expect_context::<CurrentSession>();
    let own_email = move || match session.state.get_untracked() {
        SignInState::SignedIn(account) => Some(account.email),
        _ => None,
    };

    (
        move || {
            let in_dialog =
                roster.locking.with(Option::is_some) || roster.changing_role.with(Option::is_some);
            (!in_dialog).then(|| roster.refused.get().map(problem::problem))
        },
        html::ul().class("accounts").child(move || {
            roster
                .shown()
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
                let locked = id.clone();
                lock_dialog::lock_dialog(
                    account,
                    move |typed| {
                        roster.intended(
                            &id,
                            Intent::Lock {
                                typed,
                                since: roster::now(),
                            },
                        )
                    },
                    move || roster.changing(&locked),
                    roster.refused,
                    move || {
                        roster.refused.set(None);
                        roster.locking.set(None);
                    },
                )
            })
        },
        move || {
            roster.changing_role.get().map(|account| {
                let id = account.id.clone();
                let changed = id.clone();
                let yourself = own_email().as_deref() == Some(account.email.as_str());
                let intent = if account.operator {
                    Intent::Dismiss
                } else {
                    Intent::Appoint
                };
                role_dialog::role_dialog(
                    account,
                    yourself,
                    move || {
                        let landed = roster.intended(&id, intent.clone());

                        async move {
                            let landed = landed.await;
                            if landed && yourself {
                                session.ask_again();
                            }
                            landed
                        }
                    },
                    move || roster.changing(&changed),
                    roster.refused,
                    move || {
                        roster.refused.set(None);
                        roster.changing_role.set(None);
                    },
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
    let role = (account.operator || (account.verified && account.lock.is_none())).then(|| {
        let chosen = account.clone();
        html::button()
            .r#type("button")
            .on(ev::click, move |_| {
                roster.changing_role.set(Some(chosen.clone()))
            })
            .child(speech::said(if account.operator {
                "instance-account-dismiss"
            } else {
                "instance-account-appoint"
            }))
    });
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
        html::div().class("actions").child((role, action)),
    ))
}
