use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, component, ev, html, view};
use leptos_router::components::A;
use leptos_router::hooks;

use crate::accounts::{CurrentSession, SignInState, service};
use crate::http::ApiError;
use crate::problem;
use crate::quiet_note::QuietNote;
use crate::route;
use crate::shell;
use crate::speech;
use crate::speech::Speech;

#[component]
pub fn InvitationPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let note = expect_context::<QuietNote>();
    let speech = expect_context::<Speech>();
    let navigate = hooks::use_navigate();
    let token = hooks::use_location()
        .hash
        .get_untracked()
        .trim_start_matches('#')
        .to_owned();
    let refused = RwSignal::new(None::<ApiError>);
    let answering = RwSignal::new(false);
    let signed_in_as = move || {
        let mut told = FluentArgs::new();
        if let SignInState::SignedIn(account) = session.state() {
            told.set("email", account.email);
        }

        speech::say_with("invitation-signed-in", &told)
    };
    let answer = move |token: String, accepting: bool| {
        if answering.get_untracked() {
            return;
        }
        answering.set(true);
        refused.set(None);
        let navigate = navigate.clone();
        task::spawn_local(async move {
            let answered = if accepting {
                service::take_up_invitation(&token).await
            } else {
                service::decline_invitation(&token).await
            };
            match answered {
                Ok(()) => {
                    note.tell(speech.say(if accepting {
                        "invitation-accepted"
                    } else {
                        "invitation-declined"
                    }));
                    navigate(route::WORKSPACE, Default::default());
                }
                Err(failed) => refused.set(Some(failed)),
            }
            answering.set(false);
        });
    };

    (
        shell::masthead(None),
        html::main().class("signing-in").child((
            html::h1().child(speech::said("invitation-accept-heading")),
            html::p().class("hint").child(signed_in_as),
            move || refused.get().map(problem::problem),
            (!token.is_empty()).then(|| {
                let declining = token.clone();
                let accept = answer.clone();
                html::div().class("answers").child((
                    html::button()
                        .r#type("button")
                        .class("primary")
                        .disabled(move || answering.get())
                        .on(ev::click, move |_| accept(token.clone(), true))
                        .child(speech::said("invitation-accept")),
                    html::button()
                        .r#type("button")
                        .disabled(move || answering.get())
                        .on(ev::click, move |_| answer(declining.clone(), false))
                        .child(speech::said("invitation-decline")),
                ))
            }),
            view! {
                <A href=route::WORKSPACE attr:class="back">
                    {speech::said("missing-back")}
                </A>
            },
        )),
    )
}
