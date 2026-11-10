use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, component, ev, html};
use time::Duration;

use crate::accounts::{CurrentSession, PendingDeletion, SignInState, service};
use crate::act::{Act, Here};
use crate::http::ApiError;
use crate::moment;
use crate::problem;
use crate::shell;
use crate::speech;

const KEPT_BEFORE_DELETION: Duration = Duration::days(30);

#[component]
pub fn DeletionPendingPage() -> impl IntoView {
    let session = expect_context::<CurrentSession>();
    let deletion = match session.state() {
        SignInState::DeletionPending(account) => account.deletion,
        _ => None,
    };

    (
        shell::masthead(None),
        html::main()
            .class("signing-in deletion-pending")
            .child(deletion.map(|deletion| told(session, deletion))),
    )
}

fn told(session: CurrentSession, deletion: PendingDeletion) -> impl IntoView {
    let refused = RwSignal::new(None::<ApiError>);
    let keeping = Act::new(move |(), _: Here| {
        refused.set(None);
        async move {
            match service::keep_account().await {
                Ok(account) => session.signed_in(account),
                Err(failed) => refused.set(Some(failed)),
            }
        }
    });
    let until = moment::human_day(deletion.until);
    let asked = moment::human_day(deletion.until - KEPT_BEFORE_DELETION);

    (
        html::h1().child(speech::said("deletion-pending-heading")),
        if deletion.by_the_operator {
            html::p()
                .child(move || said_with("deletion-pending-by-the-operator", &[("until", &until)]))
                .into_any()
        } else {
            (
                html::p().child(move || {
                    said_with(
                        "deletion-pending-asked",
                        &[("asked", &asked), ("until", &until)],
                    )
                }),
                html::p().child(speech::said("deletion-pending-nothing-lost")),
                move || refused.get().map(problem::problem),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .attr("aria-busy", move || keeping.busy_on(&()))
                    .on(ev::click, move |_| keeping.start(()))
                    .child(speech::said("deletion-pending-keep")),
            )
                .into_any()
        },
        html::button()
            .r#type("button")
            .class("turning")
            .on(ev::click, move |_| session.sign_out())
            .child(speech::said(if deletion.by_the_operator {
                "deletion-pending-sign-out"
            } else {
                "deletion-pending-let-go"
            })),
    )
}

fn said_with(key: &'static str, args: &[(&'static str, &String)]) -> String {
    let mut told = FluentArgs::new();
    for (name, value) in args {
        told.set(*name, (*value).clone());
    }

    speech::say_with(key, &told)
}
