use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, FocusEvent, KeyboardEvent};

use crate::outline::model::{Section, SectionId};
use crate::projects::open_project::OpenProject;
use crate::route;
use crate::scenes::model::SceneId;
use crate::scenes::prose::SyncConnection;
use crate::writing::asking::asking;
use crate::writing::beginnings::Pending;
use crate::writing::bringing_back::{Landing, bringing_back};
use crate::writing::contents::{BOOK_RAIL, contents};
use crate::writing::heading::pending_heading;
use crate::writing::heading::{first_heading, level_of, section_heading};
use crate::writing::margin::Edge;
use crate::writing::menus::{
    book_edge, insertion, open_menu_where_writing, scene_margin, section_margin,
};
use crate::writing::opening::{opening, pending_first_scene, pending_scene};
use crate::writing::placing::placing;
use crate::writing::scene_in_flow::{scene_in_flow, waiting_scene};
use crate::writing::settings_dialog::settings_dialog;
use crate::writing::side::Side;
use crate::writing::status_note::status_note;
use crate::writing::this_scene::{SCENE_TRAY, ideas_to_link, this_scene};
use crate::writing::viewport::SCENE_PLACE;
use crate::writing::write_view_state::WriteViewState;

#[component]
pub fn WriteView(project: String, following: Option<SceneId>, side: Side) -> impl IntoView {
    let state = WriteViewState::open(&route::project_id(&project), side);
    let open_project = expect_context::<OpenProject>();
    open_project.open(&route::project_id(&project));
    Effect::new(move || {
        if let Some(failure) = open_project.refused() {
            state.problems.report(failure);
        }
    });
    let switching = project.clone();
    let placed_from = project.clone();
    let segment = project.clone();
    let open = Memo::new(move |_| state.structure.is_open());
    let unshaped = Memo::new(move |_| state.structure.is_unshaped());

    let navigate = use_navigate();

    Effect::new(move |landed: Option<bool>| {
        let ready = open.get() && (side == Side::Book || state.loose.is_known());
        if landed == Some(true) || !ready {
            return landed.unwrap_or(false);
        }

        let elsewhere = following.clone().filter(|scene| {
            let in_the_book = untrack(|| state.structure.holds(scene));

            match side {
                Side::Book => !in_the_book,
                Side::Loose => in_the_book,
            }
        });

        match elsewhere {
            Some(scene) => navigate(
                &match side {
                    Side::Book => route::loose_scene_at(&project, &scene),
                    Side::Loose => route::writing_at(&project, &scene),
                },
                NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            ),
            None => {
                if following.is_some()
                    && let Ok(history) = window().history()
                {
                    let here = match side {
                        Side::Book => route::writing(&project),
                        Side::Loose => route::loose_scenes(&project),
                    };
                    let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&here));
                }
                untrack(|| state.left_off.land(following.clone()));
            }
        }

        true
    });

    let clicking = window_event_listener(ev::click, move |_| state.margin.let_go());
    let typing = window_event_listener(ev::keydown, move |event| match event.key().as_str() {
        "Escape" if within(&event, SCENE_TRAY) => state.scene_tray.close(),
        "Escape" if within(&event, BOOK_RAIL) => state.book_rail.close(),
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
        .attr("lang", move || open_project.language())
        .attr("spellcheck", move || {
            state.settings.checks_spelling().to_string()
        })
        .on(ev::focusin, move |event| {
            if let Some(section) = section_around(&event) {
                state.word_count.writing_in(section);
            }
            if let Some(scene) = scene_around(&event) {
                state.scene_tray.follow(scene);
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
            move || {
                let anything_outside = !state.loose.in_order().is_empty();

                (side == Side::Loose || anything_outside)
                    .then(|| side_switch(switching.clone(), state))
            },
            move || match side {
                Side::Book => (
                    unshaped.get().then(|| first_heading(state)),
                    open.get().then(|| book(state)),
                )
                    .into_any(),
                Side::Loose => open.get().then(|| loose_side(state)).into_any(),
            },
        ));

    (
        book_view,
        asking(state),
        bringing_back(state),
        status_note(state),
        this_scene(state, segment),
        (side == Side::Book).then(|| contents(state)),
        ideas_to_link(state),
        settings_dialog(state),
        placing(state, placed_from),
    )
}

fn scene_around(event: &FocusEvent) -> Option<SceneId> {
    event
        .target()?
        .dyn_into::<Element>()
        .ok()?
        .closest("[data-scene]")
        .ok()??
        .get_attribute("data-scene")
        .map(SceneId::from)
}

fn within(event: &KeyboardEvent, drawer: &str) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .and_then(|target| {
            target
                .closest(&format!("#{drawer}, [aria-controls={drawer}]"))
                .ok()
                .flatten()
        })
        .is_some()
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

fn side_switch(project: String, state: WriteViewState) -> impl IntoView {
    let book = route::writing(&project);
    let elsewhere = route::loose_scenes(&project);
    let loose = Memo::new(move |_| state.loose.in_order().len());

    html::nav()
        .class("side-switch")
        .attr("aria-label", "Sides of the work")
        .child((
            view! {
                <A href=book exact=true>
                    "The book"
                </A>
            },
            view! {
                <A href=elsewhere exact=true>
                    "Not in the book"
                    <span class="tally">{move || loose.get()}</span>
                </A>
            },
        ))
}

fn loose_side(state: WriteViewState) -> impl IntoView {
    let none = Memo::new(move |_| state.loose.in_order().is_empty());

    view! {
        {move || {
            none.get()
                .then(|| html::p().class("every-scene-placed").child("Every scene is in the book."))
        }}
        <For
            each=move || state.loose.in_order()
            key=|scene| scene.clone()
            let:scene
        >
            {scene_slot(None, scene, state)}
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
            move || {
                (empty.get() || state.beginnings.hands_over(&waiting))
                    .then(|| opening(waiting.clone(), state))
            },
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
            {scene_slot(Some(holding.clone()), scene, state)}
        </For>
    }
}

fn scene_slot(section: Option<SectionId>, scene: SceneId, state: WriteViewState) -> impl IntoView {
    let place: NodeRef<Div> = NodeRef::new();
    let near = Memo::new({
        let scene = scene.clone();

        move |_| state.viewport.is_near(&scene)
    });

    Effect::new(move || {
        if let Some(watched) = place.get() {
            state.viewport.watch(&watched);
        }
    });
    on_cleanup(move || {
        if let Some(watched) = place.get_untracked() {
            state.viewport.stop_watching(&watched);
        }
    });

    html::div()
        .class("scene-slot")
        .class(("waiting", move || !near.get()))
        .attr(SCENE_PLACE, scene.to_string())
        .node_ref(place)
        .child(move || {
            if !near.get() {
                return waiting_scene(scene.clone(), state).into_any();
            }

            (
                html::div().class("scene-row").child((
                    scene_margin(scene.clone(), state),
                    scene_in_flow(scene.clone(), state),
                )),
                section.clone().map(|section| {
                    (
                        pending_after(section.clone(), scene.clone(), state),
                        insertion(
                            Landing {
                                section,
                                after: Some(scene.clone()),
                            },
                            state,
                        ),
                    )
                }),
            )
                .into_any()
        })
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
