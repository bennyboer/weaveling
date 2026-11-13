use leptos::prelude::*;
use leptos::{IntoView, ev, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState};
use crate::icons;
use crate::icons::Icon;
use crate::route;
use crate::speech;

/// A note shown after signing in with a recovery code.
/// It says how many codes are left and links to setting up a new phone.
/// When the last code was used, the note is shown more urgently.
pub fn recovery_note() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    move || {
        if !session.signed_in_by_recovery_code() {
            return None;
        }
        let SignInState::SignedIn(account) = session.state() else {
            return None;
        };
        let left = account.second_factor?.recovery_codes_left;
        let last = left == 0;

        Some(
            html::div()
                .class("recovery-note")
                .class(("last", last))
                .role(if last { "alert" } else { "status" })
                .child((
                    html::span().child(if last {
                        speech::say("recovery-note-last")
                    } else {
                        speech::say_about("recovery-note", "left", &left.to_string())
                    }),
                    view! {
                        <A href=route::SECURITY on:click=move |_| session.recovery_code_noted()>
                            {speech::said("recovery-note-set-up-anew")}
                        </A>
                    },
                    html::button()
                        .r#type("button")
                        .attr("aria-label", speech::said("problem-dismiss"))
                        .attr("title", speech::said("problem-dismiss"))
                        .on(ev::click, move |_| session.recovery_code_noted())
                        .child(icons::mark(Icon::Remove)),
                )),
        )
    }
}
