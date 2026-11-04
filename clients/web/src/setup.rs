use leptos::prelude::*;
use leptos::{IntoView, component, ev, html, view};
use leptos_router::NavigateOptions;
use leptos_router::components::{A, Redirect};

use crate::accounts::{CurrentSession, SignInState, service};
use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::problem;
use crate::route;
use crate::shell;
use crate::speech;

#[component]
pub fn SetUpPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        html::main()
            .class("operating")
            .child(move || match session.state() {
                SignInState::SignedIn(account) if account.operator => {
                    view! { <Redirect path=route::INSTANCE_ACCOUNTS options=replacing() /> }
                        .into_any()
                }
                SignInState::SignedIn(_) if session.operator_wanted() => {
                    becoming(session).into_any()
                }
                SignInState::SignedIn(_) => set_up_already().into_any(),
                _ => without_accounts().into_any(),
            }),
    )
}

pub fn replacing() -> NavigateOptions {
    NavigateOptions {
        replace: true,
        ..Default::default()
    }
}

fn becoming(session: CurrentSession) -> impl IntoView {
    let code = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);

    let becoming = Act::new(move |(), _: Here| {
        refused.set(None);
        let typed = code.get_untracked();

        async move {
            match service::become_operator(&typed).await {
                Ok(()) => session.became_operator(),
                Err(failed) => refused.set(Some(failed)),
            }
        }
    });

    (
        html::h1().child(speech::said("operator-become-heading")),
        html::p()
            .class("hint")
            .child(speech::said("operator-become-hint")),
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                becoming.start(());
            })
            .child((
                html::label().class("field").child((
                    html::span().child(speech::said("operator-code")),
                    html::input()
                        .r#type("text")
                        .attr("autocomplete", "off")
                        .attr("spellcheck", "false")
                        .required(true)
                        .prop("value", move || code.get())
                        .on(ev::input, move |event| code.set(event_target_value(&event))),
                )),
                move || refused.get().map(problem::problem),
                html::button()
                    .r#type("submit")
                    .class("primary")
                    .attr("aria-busy", move || becoming.busy())
                    .child(speech::said("operator-become-confirm")),
            )),
    )
}

fn set_up_already() -> impl IntoView {
    (
        html::h1().child(speech::said("set-up-heading")),
        html::p().class("hint").child(speech::said("set-up-done")),
        back(),
    )
}

pub fn without_accounts() -> impl IntoView {
    (
        html::p()
            .class("hint")
            .child(speech::said("operator-no-accounts")),
        back(),
    )
}

fn back() -> impl IntoView {
    view! {
        <A href=route::WORKSPACE attr:class="back">
            {speech::said("missing-back")}
        </A>
    }
}
