use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::accounts::listed::ListedAccount;
use crate::accounts::roster;
use crate::accounts::roster::{Intent, Roster};
use crate::accounts::{
    CurrentSession, SignInState, lock_dialog, role_dialog, second_factor_dialog,
};
use crate::icons;
use crate::icons::Icon;
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
    let clicking_away = window_event_listener(ev::click, move |_| roster.menu.set(None));
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            roster.menu.set(None);
        }
    });
    on_cleanup(move || {
        clicking_away.remove();
        escaping.remove();
    });
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
            roster.turning_off.get().map(|account| {
                let id = account.id.clone();
                let turned = id.clone();
                second_factor_dialog::second_factor_dialog(
                    account,
                    move || roster.intended(&id, Intent::TurnOffSecondFactor),
                    move || roster.changing(&turned),
                    roster.refused,
                    move || {
                        roster.refused.set(None);
                        roster.turning_off.set(None);
                    },
                )
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

/// What an operator may do with an account, behind one button, so the row keeps room for who it is.
fn actions(roster: Roster, account: ListedAccount) -> impl IntoView {
    let may_change_role = account.operator || (account.verified && account.lock.is_none());
    let anything = may_change_role || !account.operator || account.second_factor;
    let id = account.id.clone();
    let open = Memo::new(move |_| roster.menu.get().as_deref() == Some(id.as_str()));
    let labelled = said_with(
        "instance-account-actions",
        &[("name", account.name.clone())],
    );
    let toggled = account.id.clone();

    html::div()
        .class("actions")
        .on(ev::click, |event| event.stop_propagation())
        .child(anything.then(|| {
            (
                html::button()
                    .r#type("button")
                    .class("menu-button")
                    .attr("aria-label", labelled.clone())
                    .attr("title", labelled)
                    .attr("aria-haspopup", "menu")
                    .attr("aria-expanded", move || open.get().to_string())
                    .on(ev::click, move |_| {
                        roster.menu.update(|menu| {
                            *menu =
                                (menu.as_deref() != Some(toggled.as_str())).then(|| toggled.clone())
                        })
                    })
                    .child(icons::mark(Icon::More)),
                move || {
                    open.get().then(|| {
                        html::div().class("menu").role("menu").child(menu_items(
                            roster,
                            account.clone(),
                            may_change_role,
                        ))
                    })
                },
            )
        }))
}

fn menu_items(roster: Roster, account: ListedAccount, may_change_role: bool) -> impl IntoView {
    let item = move |label: &'static str, danger: bool, chosen: Box<dyn Fn() + 'static>| {
        html::button()
            .r#type("button")
            .role("menuitem")
            .class(("danger", danger))
            .on(ev::click, move |_| {
                roster.menu.set(None);
                chosen();
            })
            .child(speech::said(label))
    };
    let role = may_change_role.then(|| {
        let chosen = account.clone();
        item(
            if account.operator {
                "instance-account-dismiss"
            } else {
                "instance-account-appoint"
            },
            false,
            Box::new(move || roster.changing_role.set(Some(chosen.clone()))),
        )
    });
    let turning_off = account.second_factor.then(|| {
        let chosen = account.clone();
        item(
            "instance-account-two-step-off",
            true,
            Box::new(move || roster.turning_off.set(Some(chosen.clone()))),
        )
    });
    let locking = (!account.operator).then(|| {
        let chosen = account.clone();
        if account.lock.is_some() {
            item(
                "instance-account-unlock",
                false,
                Box::new(move || roster.unlock(chosen.id.clone())),
            )
        } else {
            item(
                "instance-account-lock",
                true,
                Box::new(move || roster.locking.set(Some(chosen.clone()))),
            )
        }
    });

    (role, turning_off, locking)
}

fn listed(roster: Roster, account: ListedAccount) -> impl IntoView {
    let acting = actions(roster, account.clone());
    let signed_up = said_with(
        "instance-account-signed-up",
        &[("when", moment::human_time(account.signed_up_at))],
    );
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
            account.second_factor.then(|| {
                html::span()
                    .class("mark")
                    .child(speech::said("instance-account-two-step"))
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
        acting,
    ))
}
