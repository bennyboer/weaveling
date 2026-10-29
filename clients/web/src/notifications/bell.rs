use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::accounts::{CurrentSession, SignInState};
use crate::icons;
use crate::icons::Icon;
use crate::notifications::invited;
use crate::notifications::invited::Invited;
use crate::notifications::notice::{Group, Notice};
use crate::problem;
use crate::speech;
use crate::speech::Speech;

pub fn bell_in_masthead() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let signed_in = Memo::new(move |_| matches!(session.state(), SignInState::SignedIn(_)));

    move || signed_in.get().then(bell)
}

fn bell() -> impl IntoView {
    let speech = expect_context::<Speech>();
    let open = RwSignal::new(false);
    let invited = Invited::following();
    let notices = Memo::new(move |_| invited.notices());

    let clicking_away = window_event_listener(ev::click, move |_| open.set(false));
    let escaping = window_event_listener(ev::keydown, move |event| {
        if event.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        clicking_away.remove();
        escaping.remove();
    });

    let labelled = move || match notices.get().len() {
        0 => speech.say("notifications"),
        waiting => {
            let mut told = FluentArgs::new();
            told.set("count", waiting);
            speech.say_with("notifications-waiting", &told)
        }
    };

    html::div()
        .class("bell")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .r#type("button")
                .class("bell-button")
                .attr("aria-label", labelled)
                .attr("title", labelled)
                .attr("aria-haspopup", "dialog")
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::click, move |_| open.update(|open| *open = !*open))
                .child((icons::mark(Icon::Bell), move || {
                    let waiting = notices.get().len();
                    (waiting > 0).then(|| {
                        html::span()
                            .class("count")
                            .attr("aria-hidden", "true")
                            .child(waiting.to_string())
                    })
                })),
            move || {
                open.get().then(|| {
                    html::div()
                        .class("menu bell-menu")
                        .role("dialog")
                        .attr("aria-label", speech::said("notifications"))
                        .child((
                            html::h2().child(speech::said("notifications")),
                            move || invited.refused().map(problem::problem),
                            move || {
                                let held = notices.get();
                                (
                                    held.is_empty().then(|| {
                                        html::p()
                                            .class("none")
                                            .child(speech::said("notifications-none"))
                                    }),
                                    Group::ALL
                                        .into_iter()
                                        .map(|group| grouped(group, &held, invited))
                                        .collect::<Vec<_>>(),
                                )
                            },
                        ))
                })
            },
        ))
}

fn grouped(group: Group, held: &[Notice], invited: Invited) -> impl IntoView + use<> {
    let in_group: Vec<Notice> = held
        .iter()
        .filter(|notice| notice.group() == group)
        .cloned()
        .collect();

    (!in_group.is_empty()).then(|| {
        html::section().class("notices").child((
            html::h3().child(speech::said(group.message())),
            html::ul().child(
                in_group
                    .into_iter()
                    .map(|notice| match notice {
                        Notice::Invitation(invitation) => invited::row(invited, invitation),
                    })
                    .collect::<Vec<_>>(),
            ),
        ))
    })
}
