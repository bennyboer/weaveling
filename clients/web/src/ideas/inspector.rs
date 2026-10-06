use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::icons::{Icon, mark};
use crate::ideas::inspector_state::InspectorState;
use crate::inputs::typed;
use crate::route;

pub fn inspector(state: InspectorState) -> impl IntoView {
    (name_field(state), appears_in(state))
}

fn name_field(state: InspectorState) -> impl IntoView {
    html::h1().class("idea-name").child(
        html::input()
            .r#type("text")
            .attr("aria-label", "Idea name")
            .attr("placeholder", "Untitled")
            .prop("value", move || {
                state.opened().map(|idea| idea.title).unwrap_or_default()
            })
            .on(ev::change, move |event| {
                if let Some(field) = typed(&event) {
                    state.retitle(field.value());
                }
            }),
    )
}

fn appears_in(state: InspectorState) -> impl IntoView {
    html::section()
        .class("appears-in")
        .child((html::h2().child("Appears in"), move || {
            let sections = state.noting_sections();
            let passages = state.linking_passages();

            if sections.is_empty() && passages.is_empty() {
                return html::p()
                    .class("empty")
                    .child(
                        "Not in the book yet \u{2014} no section notes it, no passage draws on it.",
                    )
                    .into_any();
            }

            let project = state.project();
            let in_sections = sections
                .into_iter()
                .map(|(_, named)| place(Icon::Section, route::outline(&project), named));
            let in_passages = passages
                .into_iter()
                .map(|(id, named)| place(Icon::Passage, route::passage(&project, &id), named));

            html::ul()
                .attr("aria-label", "Appears in")
                .child(in_sections.chain(in_passages).collect::<Vec<_>>())
                .into_any()
        }))
}

fn place(icon: Icon, at: String, named: String) -> impl IntoView {
    html::li().child((
        mark(icon),
        view! {
            <A href=at attr:class="name">
                {named}
            </A>
        },
    ))
}
