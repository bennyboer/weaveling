use leptos::prelude::*;
use leptos::{IntoView, component, html, view};
use leptos_router::components::A;
use leptos_router::hooks;

use crate::blocks;
use crate::invitations::listing;
use crate::invitations::service::InvitedInto;
use crate::members::people;
use crate::members::people::People;
use crate::route;
use crate::shell;
use crate::shell::Inside;
use crate::speech;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    People,
    Invited,
}

#[component]
pub fn MembersPage() -> impl IntoView {
    members_page(Section::People)
}

#[component]
pub fn MembersInvitedPage() -> impl IntoView {
    members_page(Section::Invited)
}

fn members_page(section: Section) -> impl IntoView {
    let params = hooks::use_params_map();

    move || {
        params.read().get("project").map(|project| {
            let id = route::project_id(&project);
            let people = People::asked(id.clone());

            (
                shell::masthead(Some(Inside::of(&project, None))),
                blocks::page(
                    "members-page",
                    speech::said("members-heading"),
                    Some("members-hint"),
                    sections(project.clone(), people),
                    match section {
                        Section::People => (
                            blocks::list_card(
                                "people",
                                speech::said("members-people"),
                                inviting(project.clone(), people),
                                people::people(people),
                                (),
                            ),
                            roles(),
                        )
                            .into_any(),
                        Section::Invited => {
                            listing::invitations_listing(InvitedInto::Project(id.to_string()))
                                .into_any()
                        }
                    },
                ),
            )
        })
    }
}

fn sections(project: String, people: People) -> impl IntoView {
    let invited = route::members_invited(&project);

    blocks::tabs(
        "members-sections",
        (
            view! {
                <A href=route::members(&project) exact=true>
                    {speech::said("members-people")}
                </A>
            },
            move || {
                people.owning().then(|| {
                    view! { <A href=invited.clone()>{speech::said("members-invited")}</A> }
                })
            },
        ),
    )
}

/// The link in the head of the people list that leads to inviting someone.
/// Only owners may invite, so only they see it.
fn inviting(project: String, people: People) -> impl IntoView {
    let invited = route::members_invited(&project);

    move || {
        people.owning().then(|| {
            view! {
                <A href=invited.clone() attr:class="button invite-someone">
                    {speech::said("members-invite-someone")}
                </A>
            }
        })
    }
}

/// What each role may do, as one small card per role.
fn roles() -> impl IntoView {
    html::div().class("roles").child(blocks::cards(
        [
            ("members-role-owner", "members-role-owner-may"),
            ("members-role-author", "members-role-author-may"),
            ("members-role-reader", "members-role-reader-may"),
        ]
        .into_iter()
        .map(|(role, may)| {
            blocks::card(
                "role",
                (
                    html::h3().child(speech::said(role)),
                    html::p().class("hint").child(speech::said(may)),
                ),
            )
        })
        .collect_view(),
    ))
}
