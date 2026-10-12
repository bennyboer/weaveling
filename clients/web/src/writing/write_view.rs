use leptos::prelude::*;
use leptos::{IntoView, html};

use crate::outline::model::{Section, SectionId};
use crate::route;
use crate::writing::heading::{first_heading, section_heading};
use crate::writing::scene_in_flow::scene_in_flow;
use crate::writing::write_state::WriteState;

#[component]
pub fn WriteView(project: String) -> impl IntoView {
    let state = WriteState::open(&route::project_id(&project));
    let open = Memo::new(move |_| state.is_open());
    let unshaped = Memo::new(move |_| state.is_unshaped());

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
            move || unshaped.get().then(|| first_heading(state)),
            move || open.get().then(|| book(state)),
        ))
}

fn book(state: WriteState) -> impl IntoView {
    view! {
        <For
            each=move || state.sections()
            key=|(depth, section)| (section.id.clone(), *depth)
            let:((depth, section))
        >
            {section_in_flow(depth, section, state)}
        </For>
    }
}

fn section_in_flow(depth: usize, section: Section, state: WriteState) -> impl IntoView {
    let id = section.id.clone();

    html::section().class("in-flow").child((
        section_heading(depth, section, state),
        html::div().class("scenes").child(scenes_of(id, state)),
    ))
}

fn scenes_of(section: SectionId, state: WriteState) -> impl IntoView {
    view! {
        <For
            each=move || state.scenes_of(&section)
            key=|scene| scene.clone()
            let:scene
        >
            <div class="scene-slot">
                <hr class="between-scenes" />
                {scene_in_flow(scene, state)}
            </div>
        </For>
    }
}
