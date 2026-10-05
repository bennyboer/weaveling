use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::icons::{Icon, mark};
use crate::ideas::model::IdeaId;
use crate::passages::open_passage::OpenPassage;
use crate::route;

pub fn drawn_from(open: OpenPassage) -> impl IntoView {
    html::section().class("drawn-from").child((
        html::h2().child("Drawn from"),
        move || {
            let linked_ideas = open.linked_ideas();

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
                        .map(|idea| linked_idea(idea, open))
                        .collect::<Vec<_>>(),
                )
                .into_any()
        },
        html::button()
            .r#type("button")
            .class("link-an-idea")
            .on(ev::click, move |_| open.start_picking())
            .child((mark(Icon::Plus), "Link an idea")),
    ))
}

fn linked_idea(idea: IdeaId, open: OpenPassage) -> impl IntoView {
    let named = open.named(&idea);
    let at = route::idea(&open.project(), &idea, &named);
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
            .on(ev::click, move |_| open.unlink(idea.clone()))
            .child(mark(Icon::Remove)),
    ))
}
