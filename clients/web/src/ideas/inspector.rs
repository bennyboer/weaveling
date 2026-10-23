use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::icons;
use crate::icons::Icon;
use crate::ideas::inspector_state::InspectorState;
use crate::problem;
use crate::route;
use crate::speech;

pub fn inspector(state: InspectorState) -> impl IntoView {
    (name_field(state), appears_in(state))
}

fn name_field(state: InspectorState) -> impl IntoView {
    html::h1().class("idea-name").child(
        html::input()
            .r#type("text")
            .attr("aria-label", speech::said("inspector-idea-name"))
            .attr("placeholder", speech::said("untitled"))
            .prop("value", move || {
                state.opened().map(|idea| idea.title).unwrap_or_default()
            })
            .on(ev::change, move |event| {
                state.retitle(event_target_value(&event));
            }),
    )
}

fn appears_in(state: InspectorState) -> impl IntoView {
    html::section().class("appears-in").child((
        html::h2().child(speech::said("inspector-appears-in")),
        move || {
            let sections = state.noting_sections();
            let scenes = state.linking_scenes();

            if sections.is_empty() && scenes.is_empty() {
                return html::p()
                    .class("empty")
                    .child(speech::said("inspector-appears-nowhere"))
                    .into_any();
            }

            let project = state.project();
            let in_sections = sections
                .into_iter()
                .map(|(_, named)| place(Icon::Section, route::outline(&project), named));
            let in_scenes = scenes
                .into_iter()
                .map(|(id, named)| place(Icon::Scene, route::writing_at(&project, &id), named));

            html::ul()
                .attr("aria-label", speech::said("inspector-appears-in"))
                .child(in_sections.chain(in_scenes).collect::<Vec<_>>())
                .into_any()
        },
    ))
}

fn place(icon: Icon, at: String, named: String) -> impl IntoView {
    html::li().child((
        icons::mark(icon),
        view! {
            <A href=at attr:class="name">
                {named}
            </A>
        },
    ))
}

pub fn docked(state: InspectorState) -> impl IntoView {
    let ready = Memo::new(move |_| state.opened().is_some());

    move || {
        ready.get().then(|| {
            html::section()
                .class("docked-inspector")
                .attr("aria-label", speech::said("inspector"))
                .child((
                    move || state.problem().map(problem::problem),
                    inspector(state),
                ))
        })
    }
}
