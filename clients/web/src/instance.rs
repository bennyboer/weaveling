use leptos::prelude::*;
use leptos::{IntoView, component, view};
use leptos_router::components::{A, Redirect};

use crate::accounts::{CurrentSession, SignInState, listing};
use crate::assets::instance_storage;
use crate::blocks;
use crate::invitations;
use crate::invitations::service::InvitedInto;
use crate::route;
use crate::setup;
use crate::shell;
use crate::speech;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Accounts,
    Invitations,
    Storage,
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

#[component]
pub fn InstanceStoragePage() -> impl IntoView {
    instance_page(Section::Storage)
}

fn instance_page(section: Section) -> impl IntoView {
    let session = expect_context::<CurrentSession>();

    let operating =
        move || matches!(session.state(), SignInState::SignedIn(account) if account.operator);

    (
        shell::masthead(None),
        blocks::page(
            "instance",
            speech::said("operator-heading"),
            Some("operator-you-are"),
            move || operating().then(sections),
            move || match session.state() {
                SignInState::SignedIn(account) if account.operator => match section {
                    Section::Accounts => listing::accounts_listing().into_any(),
                    Section::Invitations => {
                        invitations::listing::invitations_listing(InvitedInto::Instance).into_any()
                    }
                    Section::Storage => instance_storage::storage_overview().into_any(),
                },
                SignInState::SignedIn(_) => {
                    view! { <Redirect path=route::SETUP options=setup::replacing() /> }.into_any()
                }
                _ => setup::without_accounts().into_any(),
            },
        ),
    )
}

fn sections() -> impl IntoView {
    blocks::tabs(
        "instance-sections",
        view! {
            <A href=route::INSTANCE_ACCOUNTS>{speech::said("instance-accounts-heading")}</A>
            <A href=route::INSTANCE_INVITATIONS>
                {speech::said("instance-invitations-heading")}
            </A>
            <A href=route::INSTANCE_STORAGE>{speech::said("storage-heading")}</A>
        },
    )
}
