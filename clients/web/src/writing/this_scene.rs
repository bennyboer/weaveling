use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::drawer::{Edge, Inside, drawer};
use crate::icons::{Icon, mark};
use crate::ideas::model::IdeaId;
use crate::route;
use crate::scenes::link_dialog::link_dialog;
use crate::scenes::model::SceneId;
use crate::writing::write_view_state::WriteViewState;

pub const SCENE_TRAY: &str = "scene-tray";

pub fn this_scene(state: WriteViewState, project: String) -> impl IntoView {
    let tray = state.scene_tray;
    let project = StoredValue::new(project);

    Effect::new(move || {
        if tray.drawer().is_open()
            && let Some(scene) = tray.scene()
        {
            state.scene_ideas.look_up(scene);
        }
    });

    drawer(
        tray.drawer(),
        Edge::Right,
        || "This scene".to_owned(),
        Inside {
            id: SCENE_TRAY,
            named: "This scene",
            called: "the tray",
            head: html::p().class("tally").child("This scene"),
            body: move || match tray.scene() {
                None => html::p()
                    .class("nothing-yet")
                    .child("Put the cursor in a scene to see it here.")
                    .into_any(),
                Some(scene) => (
                    title_field(scene.clone(), state),
                    drawn_from(scene, project, state),
                )
                    .into_any(),
            },
        },
    )
}

fn title_field(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let field: NodeRef<Input> = NodeRef::new();
    let label = Memo::new({
        let scene = scene.clone();

        move |_| state.labels.of_scene(&scene)
    });
    let titled = move || label.with(|label| label.titled.then(|| label.shown.clone()));

    let rename = {
        let scene = scene.clone();

        move || {
            if let Some(typed) = field.get_untracked() {
                state
                    .labels
                    .rename(scene.clone(), typed.value().trim().to_owned());
            }
        }
    };

    html::input()
        .r#type("text")
        .class("scene-name")
        .attr("aria-label", "Scene title")
        .attr("placeholder", move || {
            label.with(|label| match label.shown.as_str() {
                "" => "An empty scene".to_owned(),
                opening if !label.titled => opening.to_owned(),
                _ => String::new(),
            })
        })
        .prop("value", move || titled().unwrap_or_default())
        .node_ref(field)
        .on(ev::change, {
            let rename = rename.clone();

            move |_| rename()
        })
        .on(ev::keydown, move |event| match event.key().as_str() {
            "Enter" => {
                event.prevent_default();
                rename();
                if !state.editors.back_into(&scene) {
                    state.left_off.land(Some(scene.clone()));
                }
            }
            "Escape" => {
                event.prevent_default();
                event.stop_propagation();
                if let Some(typed) = field.get_untracked() {
                    typed.set_value(&titled().unwrap_or_default());
                }
                if !state.editors.back_into(&scene) {
                    state.left_off.land(Some(scene.clone()));
                }
            }
            _ => {}
        })
}

fn drawn_from(
    scene: SceneId,
    project: StoredValue<String>,
    state: WriteViewState,
) -> impl IntoView {
    html::section().class("drawn-from").child((
        html::h3().class("tally").child("Drawn from"),
        move || match state.scene_ideas.linked_to(&scene) {
            None => ().into_any(),
            Some(linked) if linked.is_empty() => html::p()
                .class("empty")
                .child("No ideas linked yet.")
                .into_any(),
            Some(linked) => html::ul()
                .attr("aria-label", "Drawn from")
                .child(
                    linked
                        .into_iter()
                        .map(|idea| linked_idea(scene.clone(), idea, project, state))
                        .collect::<Vec<_>>(),
                )
                .into_any(),
        },
        html::button()
            .r#type("button")
            .class("link-an-idea")
            .on(ev::click, move |_| state.scene_ideas.start_picking())
            .child((mark(Icon::Plus), "Link an idea")),
    ))
}

fn linked_idea(
    scene: SceneId,
    idea: IdeaId,
    project: StoredValue<String>,
    state: WriteViewState,
) -> impl IntoView {
    let named = state.scene_ideas.named(&idea);
    let at = project.with_value(|project| route::idea(project, &idea, &named));
    let shown = named.clone();

    html::li().child((
        mark(Icon::Idea),
        view! {
            <A href=at attr:class="name">
                {shown}
            </A>
        },
        html::button()
            .r#type("button")
            .attr("aria-label", format!("Unlink {named}"))
            .attr("title", format!("Unlink {named}"))
            .on(ev::click, move |_| {
                state.scene_ideas.unlink(scene.clone(), idea.clone())
            })
            .child(mark(Icon::Remove)),
    ))
}

pub fn ideas_to_link(state: WriteViewState) -> impl IntoView {
    let tray = state.scene_tray;
    let ideas = state.scene_ideas;

    move || {
        ideas.is_picking().then(|| {
            link_dialog(
                Signal::derive(move || {
                    tray.scene()
                        .map(|scene| ideas.waiting_for(&scene))
                        .unwrap_or_default()
                }),
                move |chosen| {
                    if let Some(scene) = tray.scene_untracked() {
                        ideas.link(scene, chosen);
                    }
                },
                move || ideas.stop_picking(),
            )
        })
    }
}
