use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};

use crate::accounts::listed::ListedAccount;
use crate::accounts::roster::{Intent, Roster};
use crate::accounts::{HeldKey, WaysIn, service};
use crate::dialog;
use crate::moment;
use crate::problem;
use crate::speech;

/// The sign-in way the operator is about to remove from the account.
/// The operator is asked to confirm before it is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Removing {
    Key(HeldKey),
    Totp,
}

fn naming(key: &'static str, name: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("name", name.to_owned());

    speech::say_with(key, &args)
}

/// Shows the ways an account can sign in.
///
/// Every way except the password can be removed, for an account holder who lost it.
/// Each removal must be confirmed, and the dialog says what happens after it.
pub(super) fn ways_in_dialog(
    account: ListedAccount,
    roster: Roster,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let id = StoredValue::new(account.id.clone());
    let name = StoredValue::new(account.name.clone());
    let ways = RwSignal::new(None::<WaysIn>);
    let asking = RwSignal::new(None::<Removing>);
    let look = move || {
        task::spawn_local(async move {
            match service::ways_in(&id.get_value()).await {
                Ok(seen) => _ = ways.try_set(Some(seen)),
                Err(failed) => _ = roster.refused.try_set(Some(failed)),
            }
        })
    };
    look();
    let remove = move |removing: Removing| {
        let intent = match removing {
            Removing::Key(key) => Intent::RemoveKey(key.id),
            Removing::Totp => Intent::TurnOffSecondFactor,
        };
        let landing = roster.intended(&id.get_value(), intent);
        task::spawn_local(async move {
            if landing.await {
                _ = asking.try_set(None);
                look();
            }
        });
    };
    let title = Signal::derive(move || naming("instance-ways-dialog", &name.get_value()));

    dialog::modal(
        title,
        "ways-in",
        close,
        (
            move || {
                ways.get().map(|seen| {
                    html::ul()
                        .class("ways")
                        .child(rows(&seen, asking, roster, id.get_value()))
                })
            },
            move || {
                let seen = ways.get()?;
                let removing = asking.get()?;

                Some(sure(
                    &seen,
                    removing,
                    &name.get_value(),
                    asking,
                    roster,
                    id.get_value(),
                    remove,
                ))
            },
            move || {
                asking
                    .with(Option::is_none)
                    .then(|| roster.refused.get().map(problem::problem))
            },
            html::div().class("dialog-actions").child(
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("instance-ways-close")),
            ),
        ),
    )
}

fn rows(
    seen: &WaysIn,
    asking: RwSignal<Option<Removing>>,
    roster: Roster,
    id: String,
) -> impl IntoView + use<> {
    let row = move |what: String,
                    meta: String,
                    tag: Option<&'static str>,
                    act: Option<(&'static str, String, Removing)>| {
        let id = id.clone();
        html::li().class("way").child((
            html::div().class("what").child((
                html::p().class("name").child(what),
                html::p().class("meta").child(meta),
            )),
            tag.map(|tag| html::span().class("tag").child(speech::said(tag))),
            act.map(|(label, about, removing)| {
                html::button()
                    .r#type("button")
                    .attr("aria-label", about)
                    .disabled(move || roster.changing(&id))
                    .on(ev::click, move |_| {
                        roster.refused.set(None);
                        asking.set(Some(removing.clone()));
                    })
                    .child(speech::said(label))
            }),
        ))
    };
    let password = row(
        speech::say("instance-ways-password"),
        speech::say("instance-ways-password-hint"),
        Some("instance-ways-always-kept"),
        None,
    );
    let keys = seen
        .keys
        .iter()
        .map(|key| {
            let named = speech::say_about(
                if key.signs_in_alone {
                    "security-passkey-named"
                } else {
                    "security-key-named"
                },
                "name",
                &key.name,
            );
            row(
                named.clone(),
                speech::say_about("security-key-added", "when", &moment::human_time(key.since)),
                Some(if key.signs_in_alone {
                    "instance-ways-signs-in-alone"
                } else {
                    "instance-ways-second-step"
                }),
                Some((
                    "instance-ways-remove",
                    naming("instance-ways-remove-named", &named),
                    Removing::Key(key.clone()),
                )),
            )
        })
        .collect_view();
    let app = seen.totp_since.map(|since| {
        row(
            speech::say("security-app"),
            speech::say_about("security-app-on-since", "when", &moment::human_time(since)),
            Some("instance-ways-second-step"),
            Some((
                "instance-ways-turn-off",
                speech::say("instance-ways-turn-off-app"),
                Removing::Totp,
            )),
        )
    });
    let codes = seen.recovery_codes_left.map(|left| {
        row(
            speech::say("security-codes"),
            speech::say_about("security-codes-left", "left", &left.to_string()),
            None,
            None,
        )
    });

    (password, keys, app, codes)
}

fn sure<Remove: Fn(Removing) + Copy + 'static>(
    seen: &WaysIn,
    removing: Removing,
    name: &str,
    asking: RwSignal<Option<Removing>>,
    roster: Roster,
    id: String,
    remove: Remove,
) -> impl IntoView + use<Remove> {
    let (heading, confirm) = match &removing {
        Removing::Key(key) if key.signs_in_alone => (
            naming("instance-ways-removing", &key.name),
            "instance-ways-remove-passkey",
        ),
        Removing::Key(key) => (
            naming("instance-ways-removing", &key.name),
            "instance-ways-remove-key",
        ),
        Removing::Totp => (
            speech::say("instance-ways-turning-off"),
            "instance-ways-turn-off-app",
        ),
    };

    html::section()
        .class("sure")
        .role("alertdialog")
        .attr("aria-label", heading.clone())
        .child((
            html::h3().child(heading),
            html::p().child(naming(what_follows(seen, &removing), name)),
            html::p()
                .class("hint")
                .child(speech::said("instance-ways-make-sure")),
            move || roster.refused.get().map(problem::problem),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| {
                        roster.refused.set(None);
                        asking.set(None);
                    })
                    .child(speech::said("instance-ways-keep")),
                html::button()
                    .r#type("button")
                    .class("danger")
                    .attr("aria-busy", move || roster.changing(&id).then_some("true"))
                    .on(ev::click, move |_| remove(removing.clone()))
                    .child(speech::said(confirm)),
            )),
        ))
}

/// Returns the message that says what the account is left with after the way is removed.
/// Removing a passkey does not remove a second step.
/// Removing the last second step leaves only the password.
fn what_follows(seen: &WaysIn, removing: &Removing) -> &'static str {
    let second_steps_left = seen
        .keys
        .iter()
        .filter(|key| !key.signs_in_alone && *removing != Removing::Key((*key).clone()))
        .count()
        + usize::from(seen.totp_since.is_some() && *removing != Removing::Totp);

    match removing {
        Removing::Key(key) if key.signs_in_alone && seen.recovery_codes_left.is_none() => {
            "instance-ways-still-password"
        }
        _ if second_steps_left > 0 => "instance-ways-still-second-step",
        _ => "instance-ways-password-alone",
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn key(id: &str, signs_in_alone: bool) -> HeldKey {
        HeldKey {
            id: id.to_owned(),
            name: id.to_owned(),
            since: OffsetDateTime::UNIX_EPOCH,
            signs_in_alone,
        }
    }

    fn holding(keys: Vec<HeldKey>, app: bool) -> WaysIn {
        let second_step = app || keys.iter().any(|key| !key.signs_in_alone);

        WaysIn {
            keys,
            totp_since: app.then_some(OffsetDateTime::UNIX_EPOCH),
            recovery_codes_left: second_step.then_some(10),
        }
    }

    #[test]
    fn a_passkey_removed_leaves_the_password_and_whatever_second_step_there_is() {
        // expect: with no second step, the password alone is left, as it was
        assert_eq!(
            what_follows(
                &holding(vec![key("mac", true)], false),
                &Removing::Key(key("mac", true))
            ),
            "instance-ways-still-password"
        );
        // and: beside a second step, that step stays
        assert_eq!(
            what_follows(
                &holding(vec![key("mac", true)], true),
                &Removing::Key(key("mac", true))
            ),
            "instance-ways-still-second-step"
        );
    }

    #[test]
    fn the_last_second_step_removed_leaves_the_password_alone_and_any_other_keeps_the_step() {
        // expect: the only key answering the step leaves the password alone
        assert_eq!(
            what_follows(
                &holding(vec![key("blue", false), key("mac", true)], false),
                &Removing::Key(key("blue", false))
            ),
            "instance-ways-password-alone"
        );
        // and: so does the app where no key answers
        assert_eq!(
            what_follows(&holding(vec![key("mac", true)], true), &Removing::Totp),
            "instance-ways-password-alone"
        );
        // and: a key beside the app, or the app beside a key, keeps the step
        assert_eq!(
            what_follows(
                &holding(vec![key("blue", false)], true),
                &Removing::Key(key("blue", false))
            ),
            "instance-ways-still-second-step"
        );
        assert_eq!(
            what_follows(&holding(vec![key("blue", false)], true), &Removing::Totp),
            "instance-ways-still-second-step"
        );
        // and: so does another key
        assert_eq!(
            what_follows(
                &holding(vec![key("blue", false), key("green", false)], false),
                &Removing::Key(key("blue", false))
            ),
            "instance-ways-still-second-step"
        );
    }
}
