use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, component, ev, html, view};
use leptos_router::NavigateOptions;
use leptos_router::components::{A, Redirect};

use crate::accounts::{CurrentSession, SignInState, listing, service};
use crate::http::ApiError;
use crate::invitations;
use crate::problem;
use crate::route;
use crate::shell;
use crate::speech;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Accounts,
    Invitations,
}

#[component]
pub fn InstancePage() -> impl IntoView {
    view! { <Redirect path=route::INSTANCE_ACCOUNTS options=replacing() /> }
}

#[component]
pub fn InstanceAccountsPage() -> impl IntoView {
    instance_page(Section::Accounts)
}

#[component]
pub fn InstanceInvitationsPage() -> impl IntoView {
    instance_page(Section::Invitations)
}

fn replacing() -> NavigateOptions {
    NavigateOptions {
        replace: true,
        ..Default::default()
    }
}

fn instance_page(section: Section) -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        html::main()
            .class("operating")
            .child(move || match session.state() {
                SignInState::SignedIn(account) if account.operator => operating(section).into_any(),
                SignInState::SignedIn(_) => becoming(session).into_any(),
                _ => without_accounts().into_any(),
            }),
    )
}

fn operating(section: Section) -> impl IntoView {
    (
        html::h1().child(speech::said("operator-heading")),
        html::p()
            .class("hint")
            .child(speech::said("operator-you-are")),
        sections(),
        match section {
            Section::Accounts => listing::accounts_listing().into_any(),
            Section::Invitations => invitations::listing::invitations_listing().into_any(),
        },
    )
}

fn sections() -> impl IntoView {
    html::nav()
        .class("sections")
        .attr("aria-label", move || speech::say("instance-sections"))
        .child(view! {
            <A href=route::INSTANCE_ACCOUNTS>{speech::said("instance-accounts-heading")}</A>
            <A href=route::INSTANCE_INVITATIONS>
                {speech::said("instance-invitations-heading")}
            </A>
        })
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
        task::spawn_local(async move {
            match service::become_operator(&typed).await {
                Ok(()) => session.became_operator(),
                Err(failed) => refused.set(Some(failed)),
            }
            asking.set(false);
        });
    };

    (
        html::h1().child(speech::said("operator-become-heading")),
        html::p()
            .class("hint")
            .child(speech::said("operator-become-hint")),
        html::form()
            .on(ev::submit, move |event| {
                event.prevent_default();
                become_one();
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
                    .disabled(move || asking.get())
                    .child(speech::said("operator-become-confirm")),
            )),
    )
}

fn without_accounts() -> impl IntoView {
    (
        html::p()
            .class("hint")
            .child(speech::said("operator-no-accounts")),
        view! {
            <A href=route::WORKSPACE attr:class="back">
                {speech::said("missing-back")}
            </A>
        },
    )
}
