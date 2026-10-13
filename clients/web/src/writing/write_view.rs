use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::outline::model::{Section, SectionId};
use crate::route;
use crate::scenes::model::SceneId;
use crate::writing::asking::asking;
use crate::writing::beginnings::Pending;
use crate::writing::bringing_back::{Landing, bringing_back};
use crate::writing::heading::pending_heading;
use crate::writing::heading::{first_heading, level_of, section_heading};
use crate::writing::menus::{insertion, open_menu_where_writing, scene_margin, section_margin};
use crate::writing::opening::{opening, pending_scene};
use crate::writing::scene_in_flow::scene_in_flow;
use crate::writing::write_view_state::WriteViewState;

#[component]
pub fn WriteView(project: String) -> impl IntoView {
    let state = WriteViewState::open(&route::project_id(&project));
    let open = Memo::new(move |_| state.structure.is_open());
    let unshaped = Memo::new(move |_| state.structure.is_unshaped());

    window_event_listener(ev::click, move |_| state.margin.let_go());
    window_event_listener(ev::keydown, move |event| match event.key().as_str() {
        "Escape" => state.margin.return_to_writing(),
        "F10" if event.shift_key() => open_menu_where_writing(&event, state),
        _ => {}
    });

    let book_view = html::article()
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
        ));

    (book_view, asking(state), bringing_back(state))
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
    let below = Memo::new({
        let id = id.clone();

        move |_| state.beginnings.pending_section_below(&id)
    });

    html::section().class("in-flow").child((
        html::div()
            .class("heading-row")
            .attr("data-section", id.to_string())
            .child((
                section_heading(depth, section, state),
                section_margin(id.clone(), level_of(depth), state),
            )),
        html::div().class("scenes").child(scenes_of(id, state)),
        move || empty.get().then(|| opening(waiting.clone(), state)),
        move || {
            below.get().map(|pending| match pending {
                Pending::Section { after } => {
                    let depth = state.structure.depth_of(&after);

                    pending_heading(after, depth, state).into_any()
                }
                Pending::Scene { .. } => ().into_any(),
            })
        },
    ))
}

fn scenes_of(section: SectionId, state: WriteViewState) -> impl IntoView {
    let holding = section.clone();
    let first = Landing {
        section: section.clone(),
        after: None,
    };

    view! {
        {insertion(first, state)}
        <For
            each=move || state.structure.scenes_of(&section)
            key=|scene| scene.clone()
            let:scene
        >
            <div class="scene-slot">
                <div class="scene-row">
                    {scene_margin(scene.clone(), state)}
                    {scene_in_flow(scene.clone(), state)}
                </div>
                {pending_after(holding.clone(), scene.clone(), state)}
                {insertion(
                    Landing {
                        section: holding.clone(),
                        after: Some(scene),
                    },
                    state,
                )}
            </div>
        </For>
    }
}

fn pending_after(section: SectionId, scene: SceneId, state: WriteViewState) -> impl IntoView {
    let wanted = Memo::new({
        let scene = scene.clone();

        move |_| state.beginnings.pending_scene_after(&scene).is_some()
    });

    move || {
        wanted
            .get()
            .then(|| pending_scene(section.clone(), scene.clone(), state))
    }
}
