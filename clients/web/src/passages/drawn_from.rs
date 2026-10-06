use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::icons::{Icon, mark};
use crate::ideas::model::IdeaId;
use crate::passages::passage_page_state::PassagePageState;
use crate::route;

pub fn drawn_from(state: PassagePageState) -> impl IntoView {
    html::section().class("drawn-from").child((
        html::h2().child("Drawn from"),
        move || {
            let linked_ideas = state.linked_ideas();

            if linked_ideas.is_empty() {
                return html::p()
                    .class("empty")
                    .child("No ideas linked yet.")
                    .into_any();
            }

            html::ul()
                .attr("aria-label", "Drawn from")
                .child(
                    linked_ideas
                        .into_iter()
                        .map(|idea| linked_idea(idea, state))
                        .collect::<Vec<_>>(),
                )
                .into_any()
        },
        html::button()
            .r#type("button")
            .class("link-an-idea")
            .on(ev::click, move |_| state.start_picking())
            .child((mark(Icon::Plus), "Link an idea")),
    ))
}

fn linked_idea(idea: IdeaId, state: PassagePageState) -> impl IntoView {
    let named = state.named(&idea);
    let at = route::idea(&state.project(), &idea, &named);
    let shown = named.clone();

    html::li().child((
        mark(Icon::Idea),
        view! {
            <A href=at attr:class="name">
                {shown}
            </A>
        },
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Unlink {named}"))
            .attr("title", format!("Unlink {named}"))
            .on(ev::click, move |_| state.unlink(idea.clone()))
            .child(mark(Icon::Remove)),
    ))
}
