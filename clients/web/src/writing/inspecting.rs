use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::drawer::{Edge, Inside, drawer};
use crate::icons::{Icon, mark};
use crate::ideas::model::IdeaId;
use crate::outline::model::SectionId;
use crate::route;
use crate::scenes::link_dialog::{LINKING, Wording, link_dialog};
use crate::scenes::model::SceneId;
use crate::writing::inspector::Inspected;
use crate::writing::write_view_state::WriteViewState;

pub const INSPECTOR: &str = "inspector";

pub fn inspecting(state: WriteViewState, project: String) -> impl IntoView {
    let inspector = state.inspector;
    let project = StoredValue::new(project);

    Effect::new(move || {
        if !inspector.drawer().is_open() {
            return;
        }
        match inspector.inspected() {
            Some(Inspected::Scene(scene)) => state.scene_ideas.look_up(scene),
            Some(Inspected::Section(_)) => state.scene_ideas.know_the_ideas(),
            None => {}
        }
    });
    let named = move || match inspector.inspected() {
        Some(Inspected::Section(_)) => "This section".to_owned(),
        _ => "This scene".to_owned(),
    };

    drawer(
        inspector.drawer(),
        Edge::Right,
        named,
        Inside {
            id: INSPECTOR,
            named: named.into(),
            called: "tray",
            head: html::p().class("tally").child(named),
            body: move || match inspector.inspected() {
                None => html::p()
                    .class("nothing-yet")
                    .child("Put the cursor in a scene or a heading to see it here.")
                    .into_any(),
                Some(Inspected::Scene(scene)) => (
                    title_field(scene.clone(), state),
                    drawn_from(scene, project, state),
                )
                    .into_any(),
                Some(Inspected::Section(section)) => {
                    let still_there = {
                        let section = section.clone();

                        Memo::new(move |_| state.structure.title_of(&section).is_some())
                    };

                    (move || match still_there.get() {
                        false => html::p()
                            .class("nothing-yet")
                            .child("This section was removed.")
                            .into_any(),
                        true => (
                            html::p().class("section-name").child({
                                let section = section.clone();

                                move || match state.structure.title_of(&section) {
                                    Some(title) if !title.trim().is_empty() => title,
                                    _ => "Untitled section".to_owned(),
                                }
                            }),
                            pinned(section.clone(), project, state),
                        )
                            .into_any(),
                    })
                    .into_any()
                }
            },
        },
    )
}

fn pinned(
    section: SectionId,
    project: StoredValue<String>,
    state: WriteViewState,
) -> impl IntoView {
    html::section().class("drawn-from").child((
        html::h3().class("tally").child("Pinned ideas"),
        move || {
            let ideas = state.structure.ideas_of(&section);
            match ideas.is_empty() {
                true => html::p()
                    .class("empty")
                    .child("No ideas pinned here.")
                    .into_any(),
                false => html::ul()
                    .attr("aria-label", "Pinned ideas")
                    .child(
                        ideas
                            .into_iter()
                            .map(|idea| pinned_idea(idea, project, state))
                            .collect::<Vec<_>>(),
                    )
                    .into_any(),
            }
        },
        html::button()
            .r#type("button")
            .class("link-an-idea")
            .on(ev::click, move |_| state.scene_ideas.start_picking())
            .child((mark(Icon::Plus), "Pin an idea")),
    ))
}

fn pinned_idea(idea: IdeaId, project: StoredValue<String>, state: WriteViewState) -> impl IntoView {
    move || {
        let named = state.scene_ideas.named(&idea);
        let at = project.with_value(|project| route::idea(project, &idea, &named));

        let unpinned = idea.clone();
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
                .attr("aria-label", format!("Unpin {named}"))
                .attr("title", format!("Unpin {named}"))
                .on(ev::click, move |_| state.structure.unpin(unpinned.clone()))
                .child(mark(Icon::Remove)),
        ))
    }
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

const PINNING: Wording = Wording {
    title: "Pin ideas",
    listed: "Ideas to pin",
    every_one_taken: "Every idea in this project is already pinned in the outline.",
    verb: "Pin",
};

pub fn ideas_to_choose(state: WriteViewState) -> impl IntoView {
    let inspector = state.inspector;
    let ideas = state.scene_ideas;

    move || {
        if !ideas.is_picking() {
            return None;
        }

        Some(match inspector.inspected_untracked() {
            Some(Inspected::Section(section)) => {
                let section = StoredValue::new(section);

                link_dialog(
                    PINNING,
                    Signal::derive(move || {
                        ideas
                            .every_idea()
                            .into_iter()
                            .filter(|idea| !state.structure.is_pinned(&idea.id))
                            .collect()
                    }),
                    move |chosen| state.structure.pin(section.get_value(), chosen),
                    move || ideas.stop_picking(),
                )
                .into_any()
            }
            _ => link_dialog(
                LINKING,
                Signal::derive(move || {
                    inspector
                        .scene()
                        .map(|scene| ideas.waiting_for(&scene))
                        .unwrap_or_default()
                }),
                move |chosen| {
                    if let Some(scene) = inspector.scene_untracked() {
                        ideas.link(scene, chosen);
                    }
                },
                move || ideas.stop_picking(),
            )
            .into_any(),
        })
    }
}
