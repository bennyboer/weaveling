use leptos::prelude::*;
use leptos::{IntoView, component, html, view};
use leptos_router::components::{A, Redirect};

use crate::accounts::{CurrentSession, SignInState, listing};
use crate::invitations;
use crate::route;
use crate::setup;
use crate::shell;
use crate::speech;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Accounts,
    Invitations,
}

#[component]
pub fn InstancePage() -> impl IntoView {
    view! { <Redirect path=route::INSTANCE_ACCOUNTS options=setup::replacing() /> }
}

#[component]
pub fn InstanceAccountsPage() -> impl IntoView {
    instance_page(Section::Accounts)
}

#[component]
pub fn InstanceInvitationsPage() -> impl IntoView {
    instance_page(Section::Invitations)
}

fn instance_page(section: Section) -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    (
        shell::masthead(None),
        html::main()
            .class("operating")
            .child(move || match session.state() {
                SignInState::SignedIn(account) if account.operator => operating(section).into_any(),
                SignInState::SignedIn(_) => {
                    view! { <Redirect path=route::SETUP options=setup::replacing() /> }.into_any()
                }
                _ => setup::without_accounts().into_any(),
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
