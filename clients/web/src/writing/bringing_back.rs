use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::{IntoView, ev, html};

use crate::outline::model::SectionId;
use crate::scenes::model::{Scene, SceneId};
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
    let waiting: RwSignal<Option<Vec<Scene>>> = RwSignal::new(None);
    let project = state.structure.project();

    spawn_local(async move {
        match scenes::in_project(&project).await {
            Ok(all) => {
                let outside: Vec<Scene> = all
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

    let goes = if landing.after.is_some() {
        "The one you choose goes in below this scene."
    } else {
        "The one you choose goes in at the start of this section."
    };

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
                        Some(outside) => (
                            html::p().child(format!("These scenes are not in the book. {goes}")),
                            html::ul()
                                .class("choices")
                                .attr("aria-label", "Scenes not in the book")
                                .child(
                                    outside
                                        .into_iter()
                                        .map(|scene| choice(scene, landing.clone(), state))
                                        .collect::<Vec<_>>(),
                                ),
                        )
                            .into_any(),
                    },
                    html::div().class("dialog-actions").child(
                        html::button()
                            .on(ev::click, move |_| state.margin.return_to_writing())
                            .child("Cancel"),
                    ),
                )),
        )
}

fn choice(scene: Scene, landing: Landing, state: WriteViewState) -> impl IntoView {
    let shown = scene.shown_as();
    let chosen = scene.id;

    html::li().child(
        html::button()
            .class("choice")
            .on(ev::click, move |_| {
                state.margin.return_to_writing();
                state.structure.bring_back(
                    chosen.clone(),
                    landing.section.clone(),
                    landing.after.clone(),
                );
            })
            .child(shown),
    )
}
