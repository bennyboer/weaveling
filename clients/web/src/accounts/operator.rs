use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, component, ev, html, view};
use leptos_router::components::A;

use crate::accounts::{CurrentSession, SignInState, service};
use crate::http::ApiError;
use crate::problem::problem;
use crate::route;
use crate::shell::masthead;
use crate::speech::said;

#[component]
pub fn OperatorPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        masthead(None),
        html::main()
            .class("operating")
            .child(move || match session.state.get() {
                SignInState::SignedIn(account) if account.operator => operating().into_any(),
                SignInState::SignedIn(_) => becoming(session).into_any(),
                _ => without_accounts().into_any(),
            }),
    )
}

fn operating() -> impl IntoView {
    (
        html::h1().child(said("operator-heading")),
        html::p().class("hint").child(said("operator-you-are")),
    )
}

fn becoming(session: CurrentSession) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let become_one = move || {
        if asking.get_untracked() {
            return;
        }
        asking.set(true);
        refused.set(None);
        let typed = code.get_untracked();
        spawn_local(async move {
            match service::become_operator(&typed).await {
                Ok(()) => session.became_operator(),
                Err(failed) => refused.set(Some(failed)),
            }
            asking.set(false);
        });
    };

    (
        html::h1().child(said("operator-become-heading")),
        html::p().class("hint").child(said("operator-become-hint")),
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                become_one();
            })
            .child((
                html::label().class("field").child((
                    html::span().child(said("operator-code")),
                    html::input()
                        .r#type("text")
                        .attr("autocomplete", "off")
                        .attr("spellcheck", "false")
                        .required(true)
                        .prop("value", move || code.get())
                        .on(ev::input, move |event| code.set(event_target_value(&event))),
                )),
                move || refused.get().map(problem),
                html::button()
                    .r#type("submit")
                    .class("primary")
                    .disabled(move || asking.get())
                    .child(said("operator-become-confirm")),
            )),
    )
}

fn without_accounts() -> impl IntoView {
    (
        html::p().class("hint").child(said("operator-no-accounts")),
        view! {
            <A href=route::WORKSPACE attr:class="back">
                {said("missing-back")}
            </A>
        },
    )
}
