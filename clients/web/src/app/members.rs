use leptos::prelude::*;
use leptos::{IntoView, component, html, view};
use leptos_router::components::A;
use leptos_router::hooks;

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
                html::main().class("operating members-page").child((
                    html::h1().child(speech::said("members-heading")),
                    html::p().class("hint").child(speech::said("members-hint")),
                    sections(project.clone(), people),
                    match section {
                        Section::People => (
                            people::people(people),
                            html::p().class("hint").child(speech::said("members-roles")),
                        )
                            .into_any(),
                        Section::Invited => {
                            listing::invitations_listing(InvitedInto::Project(id.to_string()))
                                .into_any()
                        }
                    },
                )),
            )
        })
    }
}

fn sections(project: String, people: People) -> impl IntoView {
    let invited = route::members_invited(&project);

    html::nav()
        .class("sections")
        .attr("aria-label", move || speech::say("members-sections"))
        .child((
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
        ))
}
