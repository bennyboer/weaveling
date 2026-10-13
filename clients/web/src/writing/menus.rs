use leptos::html::{Div, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, KeyboardEvent};

use crate::icons::{Icon, mark};
use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::writing::asking::Asking;
use crate::writing::bringing_back::Landing;
use crate::writing::margin::{Marked, Menu};
use crate::writing::write_view_state::WriteViewState;

pub fn scene_margin(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let marked = Marked::Scene(scene.clone());
    let renaming = Memo::new({
        let scene = scene.clone();

        move |_| state.margin.is_renaming(&scene)
    });
    let chosen = scene.clone();

    html::div().class("scene-margin").child((
        move || {
            if renaming.get() {
                renaming_field(scene.clone(), state).into_any()
            } else {
                scene_label(scene.clone(), state).into_any()
            }
        },
        margin_menu(
            "Scene actions",
            Icon::More,
            (marked, Menu::Changing),
            state,
            move || {
                let rename = chosen.clone();
                let take_out = chosen.clone();
                let delete = chosen.clone();

                (
                    item("Rename", false, move || {
                        state.margin.start_renaming(rename.clone())
                    }),
                    item("Take out of the book", false, move || {
                        state.margin.close();
                        state.structure.take_out(take_out.clone());
                    }),
                    item("Delete scene", true, move || {
                        state.margin.ask(Asking::DeleteScene(delete.clone()))
                    }),
                )
                    .into_any()
            },
        ),
    ))
}

pub fn section_margin(section: SectionId, level: usize, state: WriteViewState) -> impl IntoView {
    let marked = Marked::Section(section.clone());

    html::div()
        .class(format!("heading-margin level-{level}"))
        .child((margin_menu(
            "Section actions",
            Icon::More,
            (marked, Menu::Changing),
            state,
            move || {
                let removed = section.clone();

                item("Remove section", false, move || {
                    let holds_scenes = untrack(|| !state.structure.scenes_of(&removed).is_empty());

                    if holds_scenes {
                        state.margin.ask(Asking::RemoveSection(removed.clone()));
                    } else {
                        state.margin.close();
                        state.structure.remove(removed.clone());
                    }
                })
                .into_any()
            },
        ),))
}

pub fn insertion(landing: Landing, state: WriteViewState) -> impl IntoView {
    let marked = Marked::Gap(landing.clone());
    let after = landing.after.as_ref().map(ToString::to_string);
    let first_in = landing.after.is_none().then(|| landing.section.to_string());

    let below_a_scene = after.is_some();

    html::div()
        .class("insertion")
        .class(("below-a-scene", below_a_scene))
        .attr("data-after", after)
        .attr("data-first-in", first_in)
        .child((
            below_a_scene.then(|| html::hr().class("between-scenes")),
            html::div().class("reach").child(margin_menu(
                "Add here",
                Icon::Plus,
                (marked, Menu::Adding),
                state,
                move || {
                    let here = landing.clone();

                    item("Bring back a scene…", false, move || {
                        state.margin.bring_back_to(here.clone())
                    })
                    .into_any()
                },
            )),
        ))
}

fn margin_menu(
    named: &'static str,
    icon: Icon,
    (marked, menu_kind): (Marked, Menu),
    state: WriteViewState,
    items: impl Fn() -> AnyView + Send + Sync + 'static,
) -> impl IntoView {
    let open = Memo::new({
        let marked = marked.clone();

        move |_| state.margin.is_open(&marked, menu_kind)
    });
    let menu: NodeRef<Div> = NodeRef::new();

    Effect::new(move || {
        if let Some(shown) = menu.get()
            && state.margin.opened_by_keys()
        {
            focus(shown.first_element_child());
        }
    });

    html::div()
        .class("margin-actions")
        .on(ev::click, |event| event.stop_propagation())
        .child((
            html::button()
                .class("menu-button")
                .class(("dotted", matches!(icon, Icon::More)))
                .attr("aria-label", named)
                .attr("aria-haspopup", "menu")
                .attr("aria-expanded", move || open.get().to_string())
                .attr("tabindex", "-1")
                .on(ev::click, move |_| {
                    state.margin.toggle(marked.clone(), menu_kind)
                })
                .child(mark(icon)),
            move || {
                open.get().then(|| {
                    html::div()
                        .class("menu")
                        .role("menu")
                        .node_ref(menu)
                        .on(ev::keydown, step_through)
                        .child(items())
                })
            },
        ))
}

pub fn open_menu_where_writing(event: &KeyboardEvent, state: WriteViewState) {
    let Some(writing) = document()
        .active_element()
        .and_then(|at| at.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    let in_scene = writing.closest("[data-scene]").ok().flatten();
    let in_heading = writing.closest(".heading-row").ok().flatten();
    let marked = match (in_scene, in_heading) {
        (Some(scene), _) => scene
            .get_attribute("data-scene")
            .map(|id| Marked::Scene(SceneId::from(id))),
        (None, Some(heading)) => heading
            .get_attribute("data-section")
            .map(|id| Marked::Section(SectionId::from(id))),
        (None, None) => None,
    };

    if let Some(marked) = marked {
        event.prevent_default();
        state.margin.open_from(marked, writing);
    }
}

fn step_through(event: KeyboardEvent) {
    let Some(at) = document().active_element() else {
        return;
    };
    let next = match event.key().as_str() {
        "ArrowDown" => at.next_element_sibling(),
        "ArrowUp" => at.previous_element_sibling(),
        _ => return,
    };

    event.prevent_default();
    focus(next);
}

fn focus(chosen: Option<Element>) {
    if let Some(chosen) = chosen.and_then(|chosen| chosen.dyn_into::<HtmlElement>().ok()) {
        let _ = chosen.focus();
    }
}

fn item(
    named: &'static str,
    danger: bool,
    chosen: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    html::button()
        .role("menuitem")
        .class(("danger", danger))
        .on(ev::click, move |_| chosen())
        .child(named)
}

fn scene_label(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let label = Memo::new(move |_| state.labels.of_scene(&scene));

    html::p()
        .class("scene-label")
        .class(("untitled", move || label.with(|label| !label.titled)))
        .child(move || label.with(|label| label.shown.clone()))
}

fn renaming_field(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let field: NodeRef<Input> = NodeRef::new();
    let placeholder = untrack(|| state.labels.of_scene(&scene)).shown;

    Effect::new(move || {
        if let Some(shown) = field.get() {
            let _ = shown.focus();
            shown.select();
        }
    });

    let finish = {
        let scene = scene.clone();

        move || {
            if state.margin.stop_renaming(&scene)
                && let Some(shown) = field.get_untracked()
            {
                state
                    .labels
                    .rename(scene.clone(), shown.value().trim().to_owned());
                state.margin.return_to_writing();
            }
        }
    };

    html::input()
        .class("scene-label renaming")
        .attr("aria-label", "Scene title")
        .attr("placeholder", placeholder)
        .prop("value", state.labels.title_of(&scene))
        .node_ref(field)
        .on(ev::click, |event| event.stop_propagation())
        .on(ev::blur, {
            let finish = finish.clone();

            move |_| finish()
        })
        .on(ev::keydown, move |event| match event.key().as_str() {
            "Enter" => {
                event.prevent_default();
                finish();
            }
            "Escape" => {
                event.prevent_default();
                state.margin.stop_renaming(&scene);
                state.margin.return_to_writing();
            }
            _ => {}
        })
}
