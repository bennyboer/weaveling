use leptos::html::Input;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};

use crate::inputs::typed;
use crate::outline::model::SectionId;
use crate::scenes::model::{SceneId, SceneSummary};
use crate::scenes::service as scenes;
use crate::writing::write_view_state::WriteViewState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landing {
    pub section: SectionId,
    pub after: Option<SceneId>,
}

pub fn bringing_back(state: WriteViewState) -> impl IntoView {
    move || {
        state
            .margin
            .landing()
            .map(|landing| chooser(landing, state))
    }
}

fn chooser(landing: Landing, state: WriteViewState) -> impl IntoView {
    let waiting: RwSignal<Option<Vec<SceneSummary>>> = RwSignal::new(None);
    let project = state.structure.project();

    spawn_local(async move {
        match scenes::in_project(&project).await {
            Ok(all) => {
                let outside: Vec<SceneSummary> = all
                    .into_iter()
                    .filter(|scene| !state.structure.holds(&scene.id))
                    .collect();
                let _ = waiting.try_set(Some(outside));
            }
            Err(failure) => {
                state.problems.report(failure);
                state.margin.return_to_writing();
            }
        }
    });

    let finding = RwSignal::new(String::new());
    let search: NodeRef<Input> = NodeRef::new();

    Effect::new(move || {
        if let Some(field) = search.get() {
            let _ = field.focus();
        }
    });

    let matching = move || {
        let wanted = finding.get().trim().to_lowercase();

        waiting
            .get()
            .unwrap_or_default()
            .into_iter()
            .filter(|scene| scene.shown_as().to_lowercase().contains(&wanted))
            .collect::<Vec<_>>()
    };
    let first_landing = landing.clone();

    html::div()
        .class("overlay")
        .on(ev::click, move |_| state.margin.return_to_writing())
        .child(
            html::div()
                .class("dialog")
                .role("dialog")
                .attr("aria-modal", "true")
                .attr("aria-label", "Bring back a scene")
                .on(ev::click, |event| event.stop_propagation())
                .child((
                    html::h2().child("Bring back a scene"),
                    move || match waiting.get() {
                        None => html::p().class("empty").child("Looking…").into_any(),
                        Some(outside) if outside.is_empty() => html::p()
                            .class("empty")
                            .child("Every scene is in the book already.")
                            .into_any(),
                        Some(_) => {
                            let entering = first_landing.clone();

                            (
                                html::p().child(
                                    "These scenes are not in the book. The one you choose goes in where you clicked.",
                                ),
                                html::input()
                                    .r#type("search")
                                    .attr("aria-label", "Find a scene")
                                    .attr("placeholder", "Find a scene")
                                    .node_ref(search)
                                    .on(ev::input, move |event| {
                                        if let Some(field) = typed(&event) {
                                            finding.set(field.value());
                                        }
                                    })
                                    .on(ev::keydown, move |event| {
                                        if event.key() == "Enter"
                                            && let Some(first) =
                                                untrack(matching).into_iter().next()
                                        {
                                            event.prevent_default();
                                            choose(first.id, entering.clone(), state);
                                        }
                                    }),
                            )
                                .into_any()
                        }
                    },
                    move || {
                        let found = matching();

                        match waiting.with(|waiting| {
                            waiting.as_ref().is_some_and(|outside| !outside.is_empty())
                        }) {
                            false => ().into_any(),
                            true if found.is_empty() => html::p()
                                .class("empty")
                                .child("No scene matches.")
                                .into_any(),
                            true => html::ul()
                                .class("choices")
                                .attr("aria-label", "Scenes not in the book")
                                .child(
                                    found
                                        .into_iter()
                                        .map(|scene| choice(scene, landing.clone(), state))
                                        .collect::<Vec<_>>(),
                                )
                                .into_any(),
                        }
                    },
                    html::div().class("dialog-actions").child(
                        html::button()
                            .on(ev::click, move |_| state.margin.return_to_writing())
                            .child("Cancel"),
                    ),
                )),
        )
}

fn choice(scene: SceneSummary, landing: Landing, state: WriteViewState) -> impl IntoView {
    let shown = scene.shown_as();
    let chosen = scene.id;

    html::li().child(
        html::button()
            .class("choice")
            .on(ev::click, move |_| {
                choose(chosen.clone(), landing.clone(), state)
            })
            .child(shown),
    )
}

fn choose(scene: SceneId, landing: Landing, state: WriteViewState) {
    state.margin.return_to_writing();
    state
        .structure
        .bring_back(scene, landing.section, landing.after);
}
