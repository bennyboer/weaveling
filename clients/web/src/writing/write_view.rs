use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use leptos_router::NavigateOptions;
use leptos_router::components::A;
use leptos_router::hooks;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, FocusEvent, KeyboardEvent, ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::outline::model::{Section, SectionId};
use crate::problem;
use crate::projects::open_project::OpenProject;
use crate::route;
use crate::scenes::model::SceneId;
use crate::scenes::prose::SyncConnection;
use crate::speech;
use crate::writing::asking;
use crate::writing::asking::Asking;
use crate::writing::beginnings::Pending;
use crate::writing::bringing_back;
use crate::writing::bringing_back::Landing;
use crate::writing::contents;
use crate::writing::drawers;
use crate::writing::focus;
use crate::writing::going_back;
use crate::writing::heading;
use crate::writing::inspecting;
use crate::writing::inspector::Inspected;
use crate::writing::margin::Edge;
use crate::writing::menus;
use crate::writing::opening;
use crate::writing::placing;
use crate::writing::quiet_note;
use crate::writing::scene_in_flow;
use crate::writing::settings_dialog;
use crate::writing::side::Side;
use crate::writing::status_note;
use crate::writing::typewriter;
use crate::writing::viewport::SCENE_PLACE;
use crate::writing::walk::{Stop, Towards};
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

    let navigate = hooks::use_navigate();

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

    Effect::new(move || {
        if let Some((first, last)) = state.choosing.span() {
            _ = request_animation_frame(move || shown_as_selected(&first, &last));
        }
    });

    let dragging_from = StoredValue::new(None::<Stop>);
    let pressing = window_event_listener(ev::pointerdown, move |event| {
        let pressed = (event.button() == 0)
            .then(|| stop_at(event.client_x(), event.client_y()))
            .flatten();
        if event.shift_key()
            && let Some(clicked) = pressed.clone()
            && reached_by_shift_click(clicked, state)
        {
            event.prevent_default();
            return;
        }
        dragging_from.set_value(pressed);
    });
    let dragging = window_event_listener(ev::pointermove, move |event| {
        if state.moving.is_carrying() {
            state.moving.carry_to(f64::from(event.client_y()));
            return;
        }
        let Some(from) = dragging_from.get_value() else {
            return;
        };
        let Some(over) = stop_at(event.client_x(), event.client_y()) else {
            return;
        };
        if !state.choosing.is_active() {
            if over == from {
                return;
            }
            state.choosing.begin_at(from);
            crate::writing::choosing::hand_the_keys_to_the_book();
        }
        state.choosing.reach_to(over);
        drawn_again_after_the_browser(state);
    });
    let letting_go = window_event_listener(ev::pointerup, move |_| {
        if state.moving.is_carrying() {
            state.moving.put_down();
            return;
        }
        if dragging_from.get_value().is_some() && state.choosing.is_active() {
            drawn_again_after_the_browser(state);
        }
        dragging_from.set_value(None);
    });
    let cancelled = window_event_listener(ev::pointercancel, move |_| {
        state.moving.put_back();
        dragging_from.set_value(None);
    });
    let clicking = window_event_listener(ev::click, move |_| state.margin.let_go());
    let copying = window_event_listener(ev::copy, move |event| state.copying.copied(&event));
    let typing = window_event_listener(ev::keydown, move |event| {
        if state.moving.is_carrying() {
            if event.key() == "Escape" {
                event.prevent_default();
                state.moving.put_back();
            }
            return;
        }
        match state.choosing.is_active() && !state.margin.asking_untracked() {
            true if !event.default_prevented() => choosing(&event, state),
            true => {}
            false => writing(&event, state),
        }
    });
    on_cleanup(move || {
        clicking.remove();
        pressing.remove();
        dragging.remove();
        letting_go.remove();
        cancelled.remove();
        copying.remove();
        typing.remove();
        state.viewport.close();
        state.connection.with_value(SyncConnection::close);
    });

    let book_view = html::article()
        .class("writing")
        .attr("aria-label", speech::said("book"))
        .style(("--empty-scene", move || {
            as_css_text(&speech::say("scene-invitation"))
        }))
        .attr("tabindex", "-1")
        .attr("lang", move || open_project.language())
        .attr("spellcheck", move || {
            state.settings.checks_spelling().to_string()
        })
        .class(("justified", move || state.settings.justifies()))
        .class(("typewriter", move || {
            state.settings.scrolls_like_a_typewriter()
        }))
        .on(ev::focusin, move |event| {
            state.left_off.let_go_of_the_offer();
            if !is_the_book_itself(&event) {
                state.choosing.let_go();
            }
            if let Some(section) = section_around(&event) {
                state.writing_in.moved_to(section);
            }
            if let Some(scene) = scene_around(&event) {
                state.writing_in.wrote_in(Some(scene.clone()));
                state.inspector.follow(Inspected::Scene(scene));
            } else {
                state.writing_in.wrote_in(None);
                if let Some(section) = heading_of(&event) {
                    state.inspector.follow(Inspected::Section(section));
                }
            }
        })
        .child((
            move || state.problems.latest().map(problem::problem),
            move || {
                let anything_outside = !state.loose.in_order().is_empty();

                (side == Side::Loose || anything_outside)
                    .then(|| side_switch(switching.clone(), state))
            },
            move || match side {
                Side::Book => (
                    unshaped.get().then(|| heading::first_heading(state)),
                    open.get().then(|| book(state)),
                )
                    .into_any(),
                Side::Loose => open.get().then(|| loose_side(state)).into_any(),
            },
        ));

    (
        focus::focus_line(state),
        book_view,
        asking::asking(state),
        bringing_back::bringing_back(state),
        status_note::status_note(state),
        inspecting::inspecting(state, segment),
        (side == Side::Book).then(|| contents::contents(state)),
        drawers::drawer_toggles(state),
        going_back::going_back(state),
        quiet_note::quiet_note(state),
        inspecting::ideas_to_choose(state),
        settings_dialog::settings_dialog(state),
        typewriter::typewriter_scrolling(state),
        placing::placing(state, placed_from),
    )
}

fn writing(event: &KeyboardEvent, state: WriteViewState) {
    match event.key().as_str() {
        "Escape" => state.margin.return_to_writing(),
        "F10" if event.shift_key() => menus::open_menu_where_writing(event, state),
        "F" | "f" if event.shift_key() && (event.ctrl_key() || event.meta_key()) => {
            event.prevent_default();
            state.focus.toggle();
        }
        _ => {}
    }
}

fn choosing(event: &KeyboardEvent, state: WriteViewState) {
    let selection = state.choosing;
    let shifted = event.shift_key();
    let commanding = event.ctrl_key() || event.meta_key();

    match event.key().as_str() {
        "ArrowUp" | "ArrowDown" if shifted => {
            event.prevent_default();
            let towards = match event.key().as_str() {
                "ArrowUp" => Towards::Before,
                _ => Towards::After,
            };
            if let Some(reached) = selection.reach(towards) {
                bring_into_sight(&reached, state);
            }
        }
        "a" | "A" if commanding => {
            event.prevent_default();
            selection.take_in_everything();
        }
        "Delete" | "Backspace" => {
            event.prevent_default();
            state
                .margin
                .ask(removing(selection.in_reading_order(), state));
        }
        "Escape" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" => {
            event.prevent_default();
            if let Some(head) = selection.head() {
                selection.let_go();
                bring_into_sight(&head, state);
                let back = match &head {
                    Stop::Scene(scene) => state.editors.back_into(scene),
                    _ => false,
                };
                if !back && !state.walk.arrive(&head, Towards::After) {
                    state.walk.arrive_when_drawn(head);
                }
            }
        }
        _ => {}
    }
}

fn removing(chosen: Vec<Stop>, state: WriteViewState) -> Asking {
    let mut scenes = Vec::new();
    let mut sections = Vec::new();

    for stop in chosen {
        match stop {
            Stop::Scene(scene) => scenes.push(scene),
            Stop::Heading(section) | Stop::Opening(section) => {
                for within in state.structure.sections_within(&section) {
                    if !sections.contains(&within) {
                        sections.push(within);
                    }
                }
            }
        }
    }
    for section in &sections {
        for scene in untrack(|| state.structure.scenes_of(section)) {
            if !scenes.contains(&scene) {
                scenes.push(scene);
            }
        }
    }

    Asking::RemoveChosen { scenes, sections }
}

fn drawn(stop: &Stop) -> Option<Element> {
    let found = match stop {
        Stop::Scene(scene) => format!("[data-scene=\"{scene}\"]"),
        Stop::Heading(section) | Stop::Opening(section) => {
            format!(".heading-row[data-section=\"{section}\"] .heading")
        }
    };

    document().query_selector(&found).ok().flatten()
}

fn drawn_again_after_the_browser(state: WriteViewState) {
    if let Some((first, last)) = untrack(|| state.choosing.span()) {
        _ = request_animation_frame(move || shown_as_selected(&first, &last));
    }
}

fn reached_by_shift_click(clicked: Stop, state: WriteViewState) -> bool {
    if !state.choosing.is_active() {
        let Some(here) = where_the_cursor_is() else {
            return false;
        };
        if here == clicked {
            return false;
        }
        state.choosing.begin_at(here);
        crate::writing::choosing::hand_the_keys_to_the_book();
    }
    state.choosing.reach_to(clicked);

    true
}

fn where_the_cursor_is() -> Option<Stop> {
    let focused = document().active_element()?;

    if let Some(scene) = focused.closest("[data-scene]").ok().flatten() {
        return scene
            .get_attribute("data-scene")
            .map(|scene| Stop::Scene(SceneId::from(scene)));
    }

    focused
        .closest(".heading-row[data-section]")
        .ok()
        .flatten()?
        .get_attribute("data-section")
        .map(|section| Stop::Heading(SectionId::from(section)))
}

fn stop_at(x: i32, y: i32) -> Option<Stop> {
    let under = document().element_from_point(x as f32, y as f32)?;

    if let Some(scene) = under.closest("[data-scene]").ok().flatten() {
        return scene
            .get_attribute("data-scene")
            .map(|scene| Stop::Scene(SceneId::from(scene)));
    }

    under
        .closest(".heading-row[data-section]")
        .ok()
        .flatten()?
        .get_attribute("data-section")
        .map(|section| Stop::Heading(SectionId::from(section)))
}

fn is_the_book_itself(event: &FocusEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|target| target.class_list().contains("writing"))
}

fn shown_as_selected(first: &Stop, last: &Stop) {
    let (Some(from), Some(to)) = (drawn(first), drawn(last)) else {
        return;
    };
    let (Ok(spanned), Ok(Some(shown))) = (document().create_range(), window().get_selection())
    else {
        return;
    };
    if spanned.set_start_before(&from).is_ok() && spanned.set_end_after(&to).is_ok() {
        _ = shown.remove_all_ranges();
        _ = shown.add_range(&spanned);
    }
}

fn bring_into_sight(stop: &Stop, state: WriteViewState) {
    let place = match stop {
        Stop::Scene(scene) => {
            state.viewport.bring_near(scene.clone());
            document()
                .query_selector(&format!("[{SCENE_PLACE}=\"{scene}\"]"))
                .ok()
                .flatten()
        }
        heading => drawn(heading),
    };
    if let Some(place) = place {
        let nearest = ScrollIntoViewOptions::new();
        nearest.set_block(ScrollLogicalPosition::Nearest);
        place.scroll_into_view_with_scroll_into_view_options(&nearest);
    }
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

fn heading_of(event: &FocusEvent) -> Option<SectionId> {
    event
        .target()?
        .dyn_into::<Element>()
        .ok()?
        .closest(".heading-row[data-section]")
        .ok()??
        .get_attribute("data-section")
        .map(SectionId::from)
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
        {menus::book_edge(Edge::Start, state)}
        <For
            each=move || state.structure.sections()
            key=|(_, section)| section.id.clone()
            let:((_, section))
        >
            {section_in_flow(section, state)}
        </For>
        {move || (!ends_in_a_scene.get()).then(|| menus::book_edge(Edge::End, state))}
    }
}

fn side_switch(project: String, state: WriteViewState) -> impl IntoView {
    let book = route::writing(&project);
    let elsewhere = route::loose_scenes(&project);
    let loose = Memo::new(move |_| state.loose.in_order().len());

    html::nav()
        .class("side-switch")
        .attr("aria-label", speech::said("writing-sides"))
        .child((
            view! {
                <A href=book exact=true>
                    {speech::said("writing-side-book")}
                </A>
            },
            view! {
                <A href=elsewhere exact=true>
                    {speech::said("writing-side-loose")}
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
                .then(|| html::p().class("every-scene-placed").child(speech::said("writing-every-scene-placed")))
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

fn section_in_flow(section: Section, state: WriteViewState) -> impl IntoView {
    let id = section.id.clone();
    let depth = Memo::new({
        let id = id.clone();

        move |_| state.structure.depth_in_book(&id)
    });
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
                    heading::section_heading(depth, section, state),
                    menus::section_margin(id.clone(), depth, state),
                )),
            html::div().class("scenes").child(scenes_of(id, state)),
            move || {
                (empty.get() || state.beginnings.hands_over(&waiting))
                    .then(|| opening::opening(waiting.clone(), state))
            },
            move || {
                below.get().map(|pending| match pending {
                    Pending::Section { after } => {
                        let depth = state.structure.depth_of(&after);

                        heading::pending_heading(after, depth, state).into_any()
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
        {menus::insertion(first, state)}
        {move || first_wanted.get().then(|| opening::pending_first_scene(starting.clone(), state))}
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
    let near = state.viewport.watched(scene.clone(), place);

    html::div()
        .class("scene-slot")
        .class(("waiting", move || !near.get()))
        .class(("chosen", {
            let scene = scene.clone();

            move || state.choosing.holds(&Stop::Scene(scene.clone()))
        }))
        .class(("carried", {
            let scene = scene.clone();

            move || state.moving.carries(&scene)
        }))
        .attr(SCENE_PLACE, scene.to_string())
        .node_ref(place)
        .child(move || {
            if !near.get() {
                return scene_in_flow::waiting_scene(scene.clone(), state).into_any();
            }

            (
                html::div().class("scene-row").child((
                    menus::scene_margin(scene.clone(), state),
                    scene_in_flow::scene_in_flow(scene.clone(), state),
                )),
                section.clone().map(|section| {
                    (
                        pending_after(section.clone(), scene.clone(), state),
                        menus::insertion(
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
            .then(|| opening::pending_scene(section.clone(), scene.clone(), state))
    }
}

fn as_css_text(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_for_css_keeps_its_quotes_and_backslashes_inside() {
        assert_eq!(as_css_text("An empty scene"), "\"An empty scene\"");
        assert_eq!(
            as_css_text("a \"quiet\" one\\"),
            "\"a \\\"quiet\\\" one\\\\\""
        );
    }
}
