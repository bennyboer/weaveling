use leptos::prelude::*;
use leptos::{IntoView, html};

use crate::outline::model::{Section, SectionId};
use crate::route;
use crate::writing::heading::{first_heading, section_heading};
use crate::writing::opening::opening;
use crate::writing::scene_in_flow::scene_in_flow;
use crate::writing::write_view_state::WriteViewState;

#[component]
pub fn WriteView(project: String) -> impl IntoView {
    let state = WriteViewState::open(&route::project_id(&project));
    let open = Memo::new(move |_| state.structure.is_open());
    let unshaped = Memo::new(move |_| state.structure.is_unshaped());

    html::article()
        .class("writing")
        .attr("aria-label", "The book")
        .child((
            move || {
                state.problems.latest().map(|failure| {
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

fn book(state: WriteViewState) -> impl IntoView {
    view! {
        <For
            each=move || state.structure.sections()
            key=|(depth, section)| (section.id.clone(), *depth)
            let:((depth, section))
        >
            {section_in_flow(depth, section, state)}
        </For>
    }
}

fn section_in_flow(depth: usize, section: Section, state: WriteViewState) -> impl IntoView {
    let id = section.id.clone();
    let empty = Memo::new({
        let id = id.clone();

        move |_| state.structure.scenes_of(&id).is_empty()
    });
    let waiting = id.clone();

    html::section().class("in-flow").child((
        section_heading(depth, section, state),
        html::div().class("scenes").child(scenes_of(id, state)),
        move || empty.get().then(|| opening(waiting.clone(), state)),
    ))
}

fn scenes_of(section: SectionId, state: WriteViewState) -> impl IntoView {
    view! {
        <For
            each=move || state.structure.scenes_of(&section)
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
