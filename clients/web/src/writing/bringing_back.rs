use leptos::html::Input;
use leptos::prelude::*;
use leptos::task;
use leptos::{IntoView, ev, html};

use crate::dialog;
use crate::outline::model::SectionId;
use crate::scenes::model::{SceneId, SceneSummary};
use crate::scenes::service as scenes;
use crate::speech;
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

    task::spawn_local(async move {
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

    dialog::modal(
        Signal::derive(speech::said("bring-back")),
        "",
        move || state.margin.return_to_writing(),
        (
            move || match waiting.get() {
                None => html::p()
                    .class("empty")
                    .child(speech::said("looking"))
                    .into_any(),
                Some(outside) if outside.is_empty() => html::p()
                    .class("empty")
                    .child(speech::said("bring-back-none"))
                    .into_any(),
                Some(_) => {
                    let entering = first_landing.clone();

                    (
                        html::p().child(speech::said("bring-back-told")),
                        html::input()
                            .r#type("search")
                            .attr("aria-label", speech::said("bring-back-find"))
                            .attr("placeholder", speech::said("bring-back-find"))
                            .node_ref(search)
                            .on(ev::input, move |event| {
                                finding.set(event_target_value(&event));
                            })
                            .on(ev::keydown, move |event| {
                                if event.key() == "Enter"
                                    && let Some(first) = untrack(matching).into_iter().next()
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

                match waiting
                    .with(|waiting| waiting.as_ref().is_some_and(|outside| !outside.is_empty()))
                {
                    false => ().into_any(),
                    true if found.is_empty() => html::p()
                        .class("empty")
                        .child(speech::said("bring-back-no-match"))
                        .into_any(),
                    true => html::ul()
                        .class("choices")
                        .attr("aria-label", speech::said("outline-scenes-waiting"))
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
                    .child(speech::said("cancel")),
            ),
        ),
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
