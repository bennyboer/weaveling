use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev, view};
use leptos_router::components::A;

use crate::drawer;
use crate::drawer::{Edge, Inside};
use crate::icons;
use crate::icons::Icon;
use crate::ideas::model::IdeaId;
use crate::outline::model::SectionId;
use crate::route;
use crate::scenes::link_dialog;
use crate::scenes::link_dialog::{LINKING, Wording};
use crate::scenes::model::SceneId;
use crate::speech;
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
        Some(Inspected::Section(_)) => speech::say("this-section"),
        _ => speech::say("this-scene"),
    };

    drawer::drawer(
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
                    .child(speech::said("inspector-nothing-yet"))
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
                            .child(speech::said("inspector-section-removed"))
                            .into_any(),
                        true => (
                            html::p().class("section-name").child({
                                let section = section.clone();

                                move || match state.structure.title_of(&section) {
                                    Some(title) if !title.trim().is_empty() => title,
                                    _ => speech::say("heading-untitled"),
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
        html::h3()
            .class("tally")
            .child(speech::said("pinned-ideas")),
        move || {
            let ideas = state.structure.ideas_of(&section);
            match ideas.is_empty() {
                true => html::p()
                    .class("empty")
                    .child(speech::said("pinned-none"))
                    .into_any(),
                false => html::ul()
                    .attr("aria-label", speech::said("pinned-ideas"))
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
            .child((icons::mark(Icon::Plus), speech::said("pin-an-idea"))),
    ))
}

fn pinned_idea(idea: IdeaId, project: StoredValue<String>, state: WriteViewState) -> impl IntoView {
    move || {
        let named = state.scene_ideas.named(&idea);
        let at = project.with_value(|project| route::idea(project, &idea, &named));

        let unpinned = idea.clone();
        let shown = named.clone();

        html::li().child((
            icons::mark(Icon::Idea),
            view! {
                <A href=at attr:class="name">
                    {shown}
                </A>
            },
            html::button()
                .r#type("button")
                .attr(
                    "aria-label",
                    speech::say_about("section-unpin", "idea", &named),
                )
                .attr("title", speech::say_about("section-unpin", "idea", &named))
                .on(ev::click, move |_| state.structure.unpin(unpinned.clone()))
                .child(icons::mark(Icon::Remove)),
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
        .attr("aria-label", speech::said("scene-title"))
        .attr("placeholder", move || {
            label.with(|label| match label.shown.as_str() {
                "" => speech::say("scene-an-empty-one"),
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
        html::h3().class("tally").child(speech::said("drawn-from")),
        move || match state.scene_ideas.linked_to(&scene) {
            None => ().into_any(),
            Some(linked) if linked.is_empty() => html::p()
                .class("empty")
                .child(speech::said("drawn-from-none"))
                .into_any(),
            Some(linked) => html::ul()
                .attr("aria-label", speech::said("drawn-from"))
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
            .child((icons::mark(Icon::Plus), speech::said("link-an-idea"))),
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
        icons::mark(Icon::Idea),
        view! {
            <A href=at attr:class="name">
                {shown}
            </A>
        },
        html::button()
            .r#type("button")
            .attr(
                "aria-label",
                speech::say_about("unlink-idea", "idea", &named),
            )
            .attr("title", speech::say_about("unlink-idea", "idea", &named))
            .on(ev::click, move |_| {
                state.scene_ideas.unlink(scene.clone(), idea.clone())
            })
            .child(icons::mark(Icon::Remove)),
    ))
}

const PINNING: Wording = Wording {
    title: "pin-ideas",
    listed: "ideas-to-pin",
    every_one_taken: "every-idea-pinned",
    confirmed: "pin-ideas-confirm",
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

                link_dialog::link_dialog(
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
            _ => link_dialog::link_dialog(
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
