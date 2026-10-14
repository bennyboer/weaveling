use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, FocusEvent};

use crate::outline::model::{Section, SectionId};
use crate::route;
use crate::scenes::model::SceneId;
use crate::scenes::prose::SyncConnection;
use crate::writing::asking::asking;
use crate::writing::beginnings::Pending;
use crate::writing::bringing_back::{Landing, bringing_back};
use crate::writing::heading::pending_heading;
use crate::writing::heading::{first_heading, level_of, section_heading};
use crate::writing::margin::Edge;
use crate::writing::menus::{
    book_edge, insertion, open_menu_where_writing, scene_margin, section_margin,
};
use crate::writing::opening::{opening, pending_first_scene, pending_scene};
use crate::writing::scene_in_flow::scene_in_flow;
use crate::writing::status_note::status_note;
use crate::writing::write_view_state::WriteViewState;

#[component]
pub fn WriteView(project: String, following: Option<SceneId>) -> impl IntoView {
    let state = WriteViewState::open(&route::project_id(&project));
    let open = Memo::new(move |_| state.structure.is_open());
    let unshaped = Memo::new(move |_| state.structure.is_unshaped());

    let navigate = use_navigate();

    Effect::new(move |landed: Option<bool>| {
        if landed == Some(true) || !open.get() {
            return landed.unwrap_or(false);
        }

        let loose = following
            .clone()
            .filter(|scene| !untrack(|| state.structure.holds(scene)));

        match loose {
            Some(scene) => navigate(
                &route::scene(&project, &scene),
                NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            ),
            None => {
                if following.is_some()
                    && let Ok(history) = window().history()
                {
                    let _ = history.replace_state_with_url(
                        &JsValue::NULL,
                        "",
                        Some(&route::writing(&project)),
                    );
                }
                untrack(|| state.left_off.land(following.clone()));
            }
        }

        true
    });

    let clicking = window_event_listener(ev::click, move |_| state.margin.let_go());
    let typing = window_event_listener(ev::keydown, move |event| match event.key().as_str() {
        "Escape" => state.margin.return_to_writing(),
        "F10" if event.shift_key() => open_menu_where_writing(&event, state),
        _ => {}
    });
    on_cleanup(move || {
        clicking.remove();
        typing.remove();
        state.viewport.close();
        state.connection.with_value(SyncConnection::close);
    });

    let book_view = html::article()
        .class("writing")
        .attr("aria-label", "The book")
        .on(ev::focusin, move |event| {
            if let Some(section) = section_around(&event) {
                state.word_count.writing_in(section);
            }
        })
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

    (
        book_view,
        asking(state),
        bringing_back(state),
        status_note(state),
    )
}

fn section_around(event: &FocusEvent) -> Option<SectionId> {
    event
        .target()?
        .dyn_into::<Element>()
        .ok()?
        .closest("[data-section]")
        .ok()??
        .get_attribute("data-section")
        .map(SectionId::from)
}

fn book(state: WriteViewState) -> impl IntoView {
    let ends_in_a_scene = Memo::new(move |_| state.structure.ends_in_a_scene());

    view! {
        {book_edge(Edge::Start, state)}
        <For
            each=move || state.structure.sections()
            key=|(depth, section)| (section.id.clone(), *depth)
            let:((depth, section))
        >
            {section_in_flow(depth, section, state)}
        </For>
        {move || (!ends_in_a_scene.get()).then(|| book_edge(Edge::End, state))}
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

    html::section()
        .class("in-flow")
        .attr("data-section", id.to_string())
        .child((
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
    let starting = section.clone();
    let first_wanted = Memo::new({
        let section = section.clone();

        move |_| state.beginnings.pending_scene_first_in(&section)
    });

    view! {
        {insertion(first, state)}
        {move || first_wanted.get().then(|| pending_first_scene(starting.clone(), state))}
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
