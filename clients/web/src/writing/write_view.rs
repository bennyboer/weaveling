use leptos::prelude::*;
use leptos::{IntoView, html};
use leptos_router::components::A;

use crate::outline::model::{Outline, Section};
use crate::route;
use crate::writing::scene_in_flow::scene_in_flow;
use crate::writing::write_state::WriteState;

const UNTITLED: &str = "Untitled section";

#[component]
pub fn WriteView(project: String) -> impl IntoView {
    let state = WriteState::open(&route::project_id(&project));
    let outlined = route::outline(&project);

    html::article()
        .class("writing")
        .attr("aria-label", "The book")
        .child((
            move || {
                state.problem().map(|failure| {
                    html::p()
                        .class("problem")
                        .role("alert")
                        .child(failure.to_string())
                })
            },
            move || {
                let outlined = outlined.clone();

                state
                    .outline()
                    .map(|outline| book(outline, outlined, state))
            },
        ))
}

fn book(outline: Outline, outlined: String, state: WriteState) -> AnyView {
    let read = outline.in_reading_order();

    if read.is_empty() {
        return html::p()
            .class("empty")
            .child((
                "This book has no sections yet. Shape it in ",
                view! { <A href=outlined>"the outline"</A> },
                " first.",
            ))
            .into_any();
    }

    read.into_iter()
        .map(|(depth, section)| (heading(depth, section), scenes_of(section, state)))
        .collect::<Vec<_>>()
        .into_any()
}

fn scenes_of(section: &Section, state: WriteState) -> Vec<AnyView> {
    section
        .scenes()
        .into_iter()
        .enumerate()
        .flat_map(|(nth, scene)| {
            let divided = (nth > 0).then(|| html::hr().class("between-scenes").into_any());

            divided
                .into_iter()
                .chain([scene_in_flow(scene, state).into_any()])
        })
        .collect()
}

fn heading(depth: usize, section: &Section) -> AnyView {
    let shown = match section.title.trim() {
        "" => UNTITLED.to_owned(),
        titled => titled.to_owned(),
    };

    match depth {
        0 => html::h2().child(shown).into_any(),
        1 => html::h3().child(shown).into_any(),
        _ => html::h4().child(shown).into_any(),
    }
}
