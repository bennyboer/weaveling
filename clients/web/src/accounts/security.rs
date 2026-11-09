use std::future::Future;

use accounts_contract::SettingUpTotpDTO;
use leptos::prelude::*;
use leptos::{IntoView, component, ev, html, task, view};
use leptos_router::hooks;
use qrcodegen::{QrCode, QrCodeEcc};
use serde_json::Value;

use crate::accounts::authenticator::{self, NoKey};
use crate::accounts::sign_in::{code_boxes, is_whole};
use crate::accounts::{
    CurrentSession, HeldKey, PreferredWay, SecondFactorInUse, SecondStepWay, SignInState,
    key_dialogs, service,
};
use crate::act::{Act, Here};
use crate::copy_button::copy_button;
use crate::http::ApiError;
use crate::icons::{self, Icon};
use crate::moment;
use crate::problem;
use crate::route;
use crate::setup;
use crate::shell;
use crate::speech;

const QUIET_ZONE: i32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Overview,
    SettingUp(SettingUpTotpDTO),
    Keeping {
        codes: Vec<String>,
        notice: &'static str,
    },
}

#[component]
pub fn SecurityPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let step = RwSignal::new(Step::Overview);
    let turned_off = RwSignal::new(false);

    (
        shell::masthead(None),
        html::main()
            .class("operating security")
            .child(move || match step.get() {
                Step::Overview => overview(session, step, turned_off).into_any(),
                Step::SettingUp(setting_up) => {
                    setting_up_view(session, setting_up, step).into_any()
                }
                Step::Keeping { codes, notice } => keeping(codes, notice, step).into_any(),
            }),
    )
}

/// Greets one who just signed up, offering a passkey or a second step now, or leaving it for later.
fn welcome(welcomed: bool) -> impl IntoView {
    welcomed.then(|| {
        html::section().class("welcome").role("status").child((
            html::p().child(speech::said("security-welcome")),
            html::a()
                .href(route::WORKSPACE)
                .child(speech::said("security-welcome-later")),
        ))
    })
}

/// What a key's dialog is about: naming the key the browser just made, or one the account holds.
#[derive(Debug, Clone, PartialEq)]
enum KeyDialog {
    Naming(Value),
    Renaming(HeldKey),
    Removing(HeldKey),
    Promoting(HeldKey),
    Demoting(HeldKey),
}

const THE_APP: &str = "app";

fn overview(
    session: CurrentSession,
    step: RwSignal<Step>,
    turned_off: RwSignal<bool>,
) -> impl IntoView {
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(None::<Asked>);
    let menu = RwSignal::new(None::<String>);
    let keys = RwSignal::new(Vec::<HeldKey>::new());
    let dialog = RwSignal::new(None::<KeyDialog>);
    let in_dialog = RwSignal::new(None::<ApiError>);
    let said = RwSignal::new(None::<&'static str>);
    let welcomed = hooks::use_query_map()
        .read_untracked()
        .get("welcome")
        .is_some();
    let clicking_away = window_event_listener(ev::click, move |_| menu.set(None));
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            menu.set(None);
        }
    });
    let look = move || {
        task::spawn_local(async move {
            match service::keys().await {
                Ok(held) => keys.set(held),
                Err(failed) => refused.set(Some(failed)),
            }
        })
    };
    look();
    let close = move || {
        in_dialog.set(None);
        dialog.set(None);
    };
    let adding = Act::new(move |passkey: bool, _: Here| {
        refused.set(None);
        said.set(None);

        async move {
            let asked = match service::begin_adding_key(passkey, None).await {
                Ok(asked) => asked,
                Err(failed) => return refused.set(Some(failed)),
            };
            match authenticator::made(&asked).await {
                Ok(answer) => dialog.set(Some(KeyDialog::Naming(answer))),
                Err(NoKey::Cancelled) => said.set(Some("key-not-made")),
                Err(NoKey::HeldAlready) => said.set(Some("key-held-here")),
                Err(NoKey::Failed) => said.set(Some("key-failed")),
            }
        }
    });
    let naming = Act::new(move |name: String, _: Here| {
        in_dialog.set(None);
        let answer = match dialog.get_untracked() {
            Some(KeyDialog::Naming(answer)) => Some(answer),
            _ => None,
        };

        async move {
            let Some(answer) = answer else { return };
            match service::finish_adding_key(&name, answer).await {
                Ok(codes) => {
                    dialog.set(None);
                    session.look_again();
                    look();
                    match codes {
                        Some(codes) => step.set(Step::Keeping {
                            codes,
                            notice: "key-added-with-codes",
                        }),
                        None => said.set(Some("key-added")),
                    }
                }
                Err(failed) => in_dialog.set(Some(failed)),
            }
        }
    });
    let renaming = Act::new(move |name: String, _: Here| {
        in_dialog.set(None);
        let renamed = match dialog.get_untracked() {
            Some(KeyDialog::Renaming(key)) => Some(key.id),
            _ => None,
        };

        async move {
            let Some(id) = renamed else { return };
            match service::rename_key(&id, &name).await {
                Ok(()) => {
                    dialog.set(None);
                    look();
                }
                Err(failed) => in_dialog.set(Some(failed)),
            }
        }
    });
    let removing = Act::new(move |password: String, _: Here| {
        in_dialog.set(None);
        let removed = match dialog.get_untracked() {
            Some(KeyDialog::Removing(key)) => Some(key.id),
            _ => None,
        };

        async move {
            let Some(id) = removed else { return };
            match service::remove_key(&id, &password).await {
                Ok(()) => {
                    dialog.set(None);
                    said.set(Some("key-removed"));
                    session.look_again();
                    look();
                }
                Err(failed) => in_dialog.set(Some(failed)),
            }
        }
    });
    let promoting = Act::new(move |password: String, _: Here| {
        in_dialog.set(None);
        let promoted = match dialog.get_untracked() {
            Some(KeyDialog::Promoting(key)) => Some(key.id),
            _ => None,
        };

        async move {
            let Some(id) = promoted else { return };
            let asked = match service::begin_adding_key(true, Some(&id)).await {
                Ok(asked) => asked,
                Err(failed) => return in_dialog.set(Some(failed)),
            };
            let answer = match authenticator::made(&asked).await {
                Ok(answer) => answer,
                Err(not_made) => {
                    dialog.set(None);
                    return said.set(Some(match not_made {
                        NoKey::Cancelled => "key-not-made",
                        NoKey::HeldAlready => "key-held-here",
                        NoKey::Failed => "key-failed",
                    }));
                }
            };
            match service::promote_key(&id, answer, &password).await {
                Ok(()) => {
                    dialog.set(None);
                    said.set(Some("key-promoted"));
                    session.look_again();
                    look();
                }
                Err(failed) => in_dialog.set(Some(failed)),
            }
        }
    });
    let demoting = Act::new(move |password: String, _: Here| {
        in_dialog.set(None);
        let demoted = match dialog.get_untracked() {
            Some(KeyDialog::Demoting(key)) => Some(key.id),
            _ => None,
        };

        async move {
            let Some(id) = demoted else { return };
            match service::demote_key(&id, &password).await {
                Ok(codes) => {
                    dialog.set(None);
                    session.look_again();
                    look();
                    match codes {
                        Some(codes) => step.set(Step::Keeping {
                            codes,
                            notice: "key-demoted-with-codes",
                        }),
                        None => said.set(Some("key-demoted")),
                    }
                }
                Err(failed) => in_dialog.set(Some(failed)),
            }
        }
    });
    on_cleanup(move || {
        clicking_away.remove();
        escaping.remove();
    });
    let beginning = Act::new(move |(), _: Here| {
        refused.set(None);

        async move {
            match service::begin_setting_up_totp().await {
                Ok(setting_up) => step.set(Step::SettingUp(setting_up)),
                Err(failed) => refused.set(Some(failed)),
            }
        }
    });

    move || match session.state() {
        SignInState::SignedIn(account) => {
            let factor = account.second_factor;
            let app = factor.filter(|factor| factor.answered_by_totp);

            (
                move || {
                    turned_off.get().then(|| {
                        html::p()
                            .class("notice")
                            .role("status")
                            .child(speech::said("two-step-turned-off"))
                    })
                },
                html::h1().child(speech::said("security-heading")),
                welcome(welcomed),
                html::p().class("hint").child(speech::said("security-hint")),
                move || {
                    let passkeys: Vec<HeldKey> = keys
                        .get()
                        .into_iter()
                        .filter(|key| key.signs_in_alone)
                        .collect();
                    (!passkeys.is_empty()).then(|| {
                        (
                            html::h2().child(speech::said("security-signing-in")),
                            html::ul().class("ways").child(
                                passkeys
                                    .into_iter()
                                    .map(|key| key_row(key, menu, dialog))
                                    .collect_view(),
                            ),
                        )
                    })
                },
                html::h2().child(speech::said("security-second-step")),
                factor.is_none().then(|| {
                    html::p()
                        .class("hint")
                        .child(speech::said("security-second-step-hint"))
                }),
                move || {
                    said.get().map(|message| {
                        html::p()
                            .class("notice")
                            .role("status")
                            .child(speech::said(message))
                    })
                },
                factor
                    .filter(|factor| factor.answered_by_totp && factor.answered_by_key)
                    .map(|factor| preferred_way_choice(session, factor, refused)),
                html::ul().class("ways").child((
                    move || {
                        keys.get()
                            .into_iter()
                            .filter(|key| !key.signs_in_alone)
                            .map(|key| key_row(key, menu, dialog))
                            .collect_view()
                    },
                    app.map(|app| app_row(app, menu, asking, beginning)),
                    factor.map(|factor| codes_row(factor, asking)),
                )),
                move || match asking.get() {
                    Some(Asked::Drawing) => drawing(session, step, asking).into_any(),
                    Some(Asked::TurningOff) => turning_off(session, turned_off, asking).into_any(),
                    None => ().into_any(),
                },
                move || refused.get().map(problem::problem),
                html::div().class("actions").child((
                    (session.keys_offered() && authenticator::supported()).then(|| {
                        (
                            html::button()
                                .r#type("button")
                                .attr("aria-busy", move || adding.busy_on(&true))
                                .on(ev::click, move |_| adding.start(true))
                                .child(speech::said("security-add-passkey")),
                            html::button()
                                .r#type("button")
                                .attr("aria-busy", move || adding.busy_on(&false))
                                .on(ev::click, move |_| adding.start(false))
                                .child(speech::said("security-add-key")),
                        )
                    }),
                    (app.is_none() && session.totp_offered()).then(|| {
                        html::button()
                            .r#type("button")
                            .attr("aria-busy", move || beginning.busy())
                            .on(ev::click, move |_| beginning.start(()))
                            .child(speech::said("security-set-up-app"))
                    }),
                    (!session.keys_offered() && !session.totp_offered()).then(|| {
                        html::p()
                            .class("hint")
                            .child(speech::said("two-step-unavailable"))
                    }),
                )),
                move || {
                    dialog.get().map(|shown| match shown {
                        KeyDialog::Naming(_) => {
                            key_dialogs::naming_the_new_key(naming, in_dialog, close).into_any()
                        }
                        KeyDialog::Renaming(key) => {
                            key_dialogs::renaming(key.name, renaming, in_dialog, close).into_any()
                        }
                        KeyDialog::Removing(key) => key_dialogs::asking_the_password(
                            &key_dialogs::REMOVING,
                            &key.name,
                            removing,
                            in_dialog,
                            close,
                        )
                        .into_any(),
                        KeyDialog::Promoting(key) => key_dialogs::asking_the_password(
                            &key_dialogs::PROMOTING,
                            &key.name,
                            promoting,
                            in_dialog,
                            close,
                        )
                        .into_any(),
                        KeyDialog::Demoting(key) => key_dialogs::asking_the_password(
                            &key_dialogs::DEMOTING,
                            &key.name,
                            demoting,
                            in_dialog,
                            close,
                        )
                        .into_any(),
                    })
                },
            )
                .into_any()
        }
        _ => setup::without_accounts().into_any(),
    }
}

/// A menu item: its message, whether it is dangerous, and what choosing it does.
type MenuItem = (&'static str, bool, Box<dyn Fn() + Send + Sync + 'static>);

/// Which way the second step is asked first, offered once more than one way answers it.
fn preferred_way_choice(
    session: CurrentSession,
    factor: SecondFactorInUse,
    refused: RwSignal<Option<ApiError>>,
) -> impl IntoView {
    let choosing = Act::new(move |way: PreferredWay, _: Here| {
        refused.set(None);

        async move {
            match service::choose_preferred_way(way).await {
                Ok(()) => session.look_again(),
                Err(failed) => refused.set(Some(failed)),
            }
        }
    });
    let option = move |way: PreferredWay, title: &'static str, hint: &'static str| {
        html::label().class("option").child((
            html::input()
                .r#type("radio")
                .attr("name", "preferred-way")
                .prop("checked", factor.preferred_way == way)
                .on(ev::change, move |_| choosing.start(way)),
            html::span().child((
                html::span().class("title").child(speech::said(title)),
                html::span().class("hint").child(speech::said(hint)),
            )),
        ))
    };

    html::fieldset().class("preferred-way").child((
        html::legend().child(speech::said("security-preferred-way")),
        option(
            PreferredWay::Chosen(SecondStepWay::Key),
            "security-preferred-key",
            "security-preferred-key-hint",
        ),
        option(
            PreferredWay::Chosen(SecondStepWay::Totp),
            "security-preferred-app",
            "security-preferred-app-hint",
        ),
        option(
            PreferredWay::LastUsed,
            "security-preferred-last-used",
            "security-preferred-last-used-hint",
        ),
    ))
}

/// A way's ⋮, holding what can be done with it: one menu open on the page at a time.
fn way_menu(
    menu: RwSignal<Option<String>>,
    named: String,
    labelled: String,
    items: Vec<MenuItem>,
) -> impl IntoView {
    let this = named;
    let open = {
        let this = this.clone();
        Memo::new(move |_| menu.get().as_deref() == Some(this.as_str()))
    };
    let items = StoredValue::new(items);

    html::div()
        .class("way-actions")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .r#type("button")
                .class("menu-button")
                .attr("aria-label", labelled.clone())
                .attr("title", labelled)
                .attr("aria-haspopup", "menu")
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::click, move |_| {
                    menu.update(|shown| {
                        *shown = (shown.as_deref() != Some(this.as_str())).then(|| this.clone())
                    })
                })
                .child(icons::mark(Icon::More)),
            move || {
                open.get().then(|| {
                    html::div()
                        .class("menu")
                        .role("menu")
                        .child(items.with_value(|listed| {
                            listed
                                .iter()
                                .enumerate()
                                .map(|(at, (label, danger, _))| {
                                    html::button()
                                        .r#type("button")
                                        .role("menuitem")
                                        .class(("danger", *danger))
                                        .on(ev::click, move |_| {
                                            menu.set(None);
                                            items.with_value(|listed| (listed[at].2)());
                                        })
                                        .child(speech::said(label))
                                })
                                .collect_view()
                        }))
                })
            },
        ))
}

fn key_row(
    key: HeldKey,
    menu: RwSignal<Option<String>>,
    dialog: RwSignal<Option<KeyDialog>>,
) -> impl IntoView {
    let renamed = key.clone();
    let removed = key.clone();
    let changed = key.clone();
    let (named, changing, change): (&str, &'static str, KeyDialog) = if key.signs_in_alone {
        (
            "security-passkey-named",
            "key-demote",
            KeyDialog::Demoting(changed),
        )
    } else {
        (
            "security-key-named",
            "key-promote",
            KeyDialog::Promoting(changed),
        )
    };

    html::li().class("way").child((
        html::div().class("what").child((
            html::p()
                .class("name")
                .child(speech::say_about(named, "name", &key.name)),
            html::p().class("meta").child(speech::say_about(
                "security-key-added",
                "when",
                &moment::human_time(key.since),
            )),
        )),
        way_menu(
            menu,
            key.id.clone(),
            speech::say_about("security-key-actions", "name", &key.name),
            vec![
                (
                    "key-rename",
                    false,
                    Box::new(move || dialog.set(Some(KeyDialog::Renaming(renamed.clone())))),
                ),
                (
                    changing,
                    false,
                    Box::new(move || dialog.set(Some(change.clone()))),
                ),
                (
                    "key-remove",
                    true,
                    Box::new(move || dialog.set(Some(KeyDialog::Removing(removed.clone())))),
                ),
            ],
        ),
    ))
}

fn app_row(
    app: SecondFactorInUse,
    menu: RwSignal<Option<String>>,
    asking: RwSignal<Option<Asked>>,
    beginning: Act<()>,
) -> impl IntoView {
    html::li().class("way").child((
        html::div().class("what").child((
            html::p().class("name").child(speech::said("security-app")),
            html::p().class("meta").child(speech::say_about(
                "security-app-on-since",
                "when",
                &moment::human_time(app.since),
            )),
        )),
        way_menu(
            menu,
            THE_APP.to_owned(),
            speech::say("security-app-actions"),
            vec![
                (
                    "two-step-set-up-anew",
                    false,
                    Box::new(move || beginning.start(())),
                ),
                (
                    "two-step-turn-off",
                    true,
                    Box::new(move || asking.set(Some(Asked::TurningOff))),
                ),
            ],
        ),
    ))
}

fn codes_row(factor: SecondFactorInUse, asking: RwSignal<Option<Asked>>) -> impl IntoView {
    html::li().class("way").child((
        html::div().class("what").child((
            html::p()
                .class("name")
                .child(speech::said("security-codes")),
            html::p().class("meta").child(speech::say_about(
                "security-codes-left",
                "left",
                &factor.recovery_codes_left.to_string(),
            )),
        )),
        html::div().class("way-actions").child(
            html::button()
                .r#type("button")
                .on(ev::click, move |_| asking.set(Some(Asked::Drawing)))
                .child(speech::said("two-step-draw-codes")),
        ),
    ))
}

fn setting_up_view(
    session: CurrentSession,
    setting_up: SettingUpTotpDTO,
    step: RwSignal<Step>,
) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let finishing = Act::new(move |(), _: Here| {
        refused.set(None);
        let typed = code.get_untracked();

        async move {
            match service::finish_setting_up_totp(&typed).await {
                Ok(codes) => {
                    session.look_again();
                    step.set(Step::Keeping {
                        codes,
                        notice: "two-step-on-now",
                    });
                }
                Err(failed) => {
                    code.set(String::new());
                    refused.set(Some(failed));
                }
            }
        }
    });
    let secret = setting_up.secret.clone();

    Effect::new(move |_| {
        if is_whole(&code.get()) {
            finishing.start(());
        }
    });

    (
        html::h1().child(speech::said("two-step-set-up-heading")),
        html::p()
            .class("hint")
            .child(speech::said("two-step-set-up-hint")),
        html::div().class("steps").child((
            html::section().class("scanning").child((
                html::h2().child(speech::said("two-step-scan")),
                html::div().class("scan").child((
                    qr_code(&setting_up.otpauth_uri),
                    html::div().class("typing").child((
                        html::p()
                            .class("hint")
                            .child(speech::said("two-step-cannot-scan")),
                        html::code().class("key").child(grouped(&setting_up.secret)),
                        copy_button("two-step-copy-key", secret),
                    )),
                )),
            )),
            html::section().class("confirming").child((
                html::h2().child(speech::said("two-step-code")),
                html::form()
                    .on(ev::submit, move |event| {
                        event.prevent_default();
                        finishing.start(());
                    })
                    .child((
                        code_boxes(speech::said("code-field"), code),
                        move || refused.get().map(problem::problem),
                        html::div().class("buttons").child((
                            html::button()
                                .r#type("button")
                                .on(ev::click, move |_| step.set(Step::Overview))
                                .child(speech::said("two-step-cancel")),
                            html::button()
                                .r#type("submit")
                                .class("primary")
                                .attr("aria-busy", move || finishing.busy())
                                .child(speech::said("two-step-confirm")),
                        )),
                        html::p()
                            .class("hint")
                            .child(speech::said("two-step-nothing-yet")),
                    )),
            )),
        )),
    )
}

/// A change that asks for the password first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    Drawing,
    TurningOff,
}

/// The words of a change asked for by the password: what it does, and the button that does it.
struct PasswordAsked {
    hint: &'static str,
    confirming: &'static str,
    danger: bool,
}

const DRAWING: PasswordAsked = PasswordAsked {
    hint: "two-step-draw-codes-hint",
    confirming: "two-step-draw-codes",
    danger: false,
};

const TURNING_OFF: PasswordAsked = PasswordAsked {
    hint: "two-step-turn-off-hint",
    confirming: "two-step-turn-off",
    danger: true,
};

fn drawing(
    session: CurrentSession,
    step: RwSignal<Step>,
    asking: RwSignal<Option<Asked>>,
) -> impl IntoView {
    asking_the_password(&DRAWING, asking, move |password| async move {
        let codes = service::draw_recovery_codes(&password).await?;
        session.look_again();
        step.set(Step::Keeping {
            codes,
            notice: "two-step-codes-drawn",
        });

        Ok(())
    })
}

fn turning_off(
    session: CurrentSession,
    turned_off: RwSignal<bool>,
    asking: RwSignal<Option<Asked>>,
) -> impl IntoView {
    asking_the_password(&TURNING_OFF, asking, move |password| async move {
        service::turn_off_totp(&password).await?;
        asking.set(None);
        turned_off.set(true);
        session.look_again();

        Ok(())
    })
}

fn asking_the_password<Fu>(
    words: &'static PasswordAsked,
    asking: RwSignal<Option<Asked>>,
    answer: impl Fn(String) -> Fu + 'static,
) -> impl IntoView
where
    Fu: Future<Output = Result<(), ApiError>> + 'static,
{
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let answering = Act::new(move |(), _: Here| {
        refused.set(None);
        let answered = answer(password.get_untracked());

        async move {
            if let Err(failed) = answered.await {
                refused.set(Some(failed));
            }
        }
    });

    html::section().class("asking").child(
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                answering.start(());
            })
            .child((
                html::p().class("hint").child(speech::said(words.hint)),
                html::label().class("field").child((
                    html::span().child(speech::said("security-password")),
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
                html::div().class("buttons").child((
                    html::button()
                        .r#type("button")
                        .on(ev::click, move |_| asking.set(None))
                        .child(speech::said("two-step-cancel")),
                    html::button()
                        .r#type("submit")
                        .class(if words.danger { "danger" } else { "primary" })
                        .attr("aria-busy", move || answering.busy())
                        .child(speech::said(words.confirming)),
                )),
            )),
    )
}

fn keeping(codes: Vec<String>, notice: &'static str, step: RwSignal<Step>) -> impl IntoView {
    let kept = RwSignal::new(false);
    let written = codes.join("\n");
    let to_copy = written.clone();

    (
        html::p()
            .class("notice")
            .role("status")
            .child(speech::said(notice)),
        html::h1().child(speech::said("two-step-codes-heading")),
        html::p()
            .class("hint")
            .child(speech::said("two-step-codes-hint")),
        html::ol().class("recovery-codes").child(
            codes
                .into_iter()
                .map(|code| html::li().child(code))
                .collect_view(),
        ),
        html::div().class("buttons").child((
            html::a()
                .class("button")
                .attr(
                    "href",
                    format!(
                        "data:text/plain;charset=utf-8,{}",
                        String::from(js_sys::encode_uri_component(&format!("{written}\n")))
                    ),
                )
                .attr("download", "weaveling-recovery-codes.txt")
                .child(speech::said("two-step-codes-download")),
            copy_button("two-step-codes-copy", to_copy),
            html::button()
                .r#type("button")
                .on(ev::click, |_| _ = window().print())
                .child(speech::said("two-step-codes-print")),
        )),
        html::div().class("done").child((
            html::label().class("kept").child((
                html::input()
                    .r#type("checkbox")
                    .prop("checked", move || kept.get())
                    .on(ev::change, move |event| {
                        kept.set(event_target_checked(&event))
                    }),
                html::span().child(speech::said("two-step-codes-kept")),
            )),
            html::button()
                .r#type("button")
                .class("primary")
                .prop("disabled", move || !kept.get())
                .on(ev::click, move |_| step.set(Step::Overview))
                .child(speech::said("two-step-done")),
        )),
    )
}

fn qr_code(text: &str) -> Option<impl IntoView + use<>> {
    let drawn = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let size = drawn.size();
    let side = size + 2 * QUIET_ZONE;
    let dark: String = (0..size)
        .flat_map(|y| (0..size).map(move |x| (x, y)))
        .filter(|&(x, y)| drawn.get_module(x, y))
        .map(|(x, y)| format!("M{} {}h1v1h-1z", x + QUIET_ZONE, y + QUIET_ZONE))
        .collect();

    Some(view! {
        <svg
            class="qr-code"
            viewBox=format!("0 0 {side} {side}")
            role="img"
            aria-label=speech::said("two-step-qr")
        >
            <path d=dark />
        </svg>
    })
}

fn grouped(secret: &str) -> String {
    secret
        .as_bytes()
        .chunks(4)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_to_type_is_shown_in_groups_of_four() {
        // expect: 32 characters read as eight groups
        assert_eq!(
            grouped("JBSWY3DPEHPK3PXPKZ4M2QHTJBSWY3DP"),
            "JBSW Y3DP EHPK 3PXP KZ4M 2QHT JBSW Y3DP"
        );
    }
}
