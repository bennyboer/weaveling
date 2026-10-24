use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, component, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState};
use crate::route;
use crate::shell;
use crate::speech;

#[component]
pub fn InvitationPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let signed_in_as = move || {
        let mut told = FluentArgs::new();
        if let SignInState::SignedIn(account) = session.state() {
            told.set("email", account.email);
        }

        speech::say_with("invitation-signed-in", &told)
    };

    (
        shell::masthead(None),
        html::main().class("signing-in").child((
            html::h1().child(speech::said("invitation-accept-heading")),
            html::p().class("hint").child(signed_in_as),
            view! {
                <A href=route::WORKSPACE attr:class="back">
                    {speech::said("missing-back")}
                </A>
            },
        )),
    )
}
