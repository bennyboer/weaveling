use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, component, ev, html};

use crate::accounts::{CurrentSession, service};
use crate::http::ApiError;
use crate::problem::problem;
use crate::shell::masthead;
use crate::speech::said;

#[component]
pub fn SignInPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let refused = RwSignal::new(None::<ApiError>);
    let asking = RwSignal::new(false);

    let sign_in = move || {
        if asking.get_untracked() {
            return;
        }
        asking.set(true);
        refused.set(None);
        let (given, typed) = (email.get_untracked(), password.get_untracked());

        spawn_local(async move {
            match service::sign_in(&given, &typed).await {
                Ok(account) => session.signed_in(account),
                Err(failed) => refused.set(Some(failed)),
            }
            asking.set(false);
        });
    };

    (
        masthead(None),
        html::main().class("session-in").child((
            html::h1().child(said("sign-in-heading")),
            html::form()
                .on(ev::submit, move |event| {
                    event.prevent_default();
                    sign_in();
                })
                .child((
                    field(said("sign-in-email"), "email", "username", email),
                    field(
                        said("sign-in-password"),
                        "password",
                        "current-password",
                        password,
                    ),
                    move || refused.get().map(problem),
                    html::button()
                        .r#type("submit")
                        .class("primary")
                        .disabled(move || asking.get())
                        .child(said("sign-in-confirm")),
                )),
        )),
    )
}

fn field(
    label: impl Fn() -> String + Send + Sync + 'static,
    kind: &'static str,
    autocomplete: &'static str,
    value: RwSignal<String>,
) -> impl IntoView {
    html::label().class("field").child((
        html::span().child(label),
        html::input()
            .r#type(kind)
            .attr("autocomplete", autocomplete)
            .required(true)
            .prop("value", move || value.get())
            .on(ev::input, move |event| {
                value.set(event_target_value(&event))
            }),
    ))
}
