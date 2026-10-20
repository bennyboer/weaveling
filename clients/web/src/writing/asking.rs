use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;

use crate::dialog::modal;
use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::speech::{said, say, say_about, say_with};
use crate::writing::beginnings::Unplaced;
use crate::writing::heading::KeptTitle;
use crate::writing::side::Side;
use crate::writing::write_view_state::WriteViewState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asking {
    DeleteScene(SceneId),
    RemoveSection(SectionId),
    RemoveChosen {
        scenes: Vec<SceneId>,
        sections: Vec<SectionId>,
    },
    RemovedMeanwhile {
        title: String,
        scene: SceneId,
    },
    PlaceGone(Unplaced),
    RenamedMeanwhile {
        section: SectionId,
        mine: String,
        theirs: String,
        kept: KeptTitle,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Confirmed,
    TakenOut,
    DeletedForGood,
    KeptMine,
    TakenTheirs,
    ShownElsewhere,
    PutAtTheEnd,
    Discarded,
}

pub fn asking(state: WriteViewState) -> impl IntoView {
    let navigate = StoredValue::new_local(use_navigate());
    Effect::new(move || {
        if let Some(unplaced) = state.beginnings.unplaced() {
            state.beginnings.let_unplaced_be_asked();
            state.margin.ask(Asking::PlaceGone(unplaced));
        }
    });

    move || {
        state.margin.asking().map(|question| {
            let (title, told) = worded(&question, state);
            let question = StoredValue::new(question);
            let choices = untrack(|| question.with_value(|question| offered(question, state)));
            let cancel = move || state.margin.return_to_writing();

            modal(
                say(title),
                "",
                cancel,
                (
                    html::p().child(told),
                    html::div().class("dialog-actions").child((
                        untrack(|| question.with_value(way_back)).map(|way_back| {
                            html::button()
                                .on(ev::click, move |_| cancel())
                                .child(said(way_back))
                        }),
                        choices
                            .into_iter()
                            .map(|(label, answer)| {
                                html::button()
                                    .class(("danger", answer == Answer::DeletedForGood))
                                    .on(ev::click, move |_| {
                                        state.margin.return_to_writing();
                                        answered(question.get_value(), answer, state, |to| {
                                            navigate.with_value(|navigate| {
                                                navigate(to, NavigateOptions::default())
                                            })
                                        });
                                    })
                                    .child(said(label))
                            })
                            .collect::<Vec<_>>(),
                    )),
                ),
            )
        })
    }
}

fn way_back(question: &Asking) -> Option<&'static str> {
    match question {
        Asking::RemovedMeanwhile { .. } => Some("asked-keep-writing"),
        Asking::PlaceGone(_) => None,
        _ => Some("cancel"),
    }
}

fn loose_and_at(scene: &SceneId) -> String {
    let path = window().location().pathname().unwrap_or_default();
    let book = path.trim_end_matches('/').trim_end_matches("/loose");

    format!("{book}/loose?scene={scene}")
}

fn offered(question: &Asking, state: WriteViewState) -> Vec<(&'static str, Answer)> {
    match question {
        Asking::DeleteScene(_) => vec![("answer-delete", Answer::DeletedForGood)],
        Asking::RemoveSection(_) => vec![("answer-remove", Answer::Confirmed)],
        Asking::RemovedMeanwhile { .. } => vec![("answer-show-it", Answer::ShownElsewhere)],
        Asking::PlaceGone(_) => vec![
            ("answer-discard", Answer::Discarded),
            ("answer-put-at-end", Answer::PutAtTheEnd),
        ],
        Asking::RenamedMeanwhile { .. } => vec![
            ("renamed-meanwhile-theirs", Answer::TakenTheirs),
            ("renamed-meanwhile-mine", Answer::KeptMine),
        ],
        Asking::RemoveChosen { .. } => match state.side {
            Side::Book => vec![
                ("answer-take-out", Answer::TakenOut),
                ("answer-delete-for-good", Answer::DeletedForGood),
            ],
            Side::Loose => vec![("answer-delete-for-good", Answer::DeletedForGood)],
        },
    }
}

fn worded(question: &Asking, state: WriteViewState) -> (&'static str, String) {
    match question {
        Asking::DeleteScene(scene) => {
            let shown = untrack(|| state.labels.of_scene(scene)).shown;
            let told = match shown.trim() {
                "" => say("asked-delete-scene-told-unnamed"),
                shown => say_about("asked-delete-scene-told", "scene", shown),
            };

            ("asked-delete-scene", told)
        }
        Asking::RemovedMeanwhile { title, scene } => {
            let shown = untrack(|| state.labels.of_scene(scene)).shown;
            let mut args = FluentArgs::new();
            args.set("section", title.trim().to_owned());
            let told = match shown.trim() {
                "" => say_with("removed-meanwhile-told-unnamed", &args),
                shown => {
                    args.set("scene", shown.to_owned());

                    say_with("removed-meanwhile-told", &args)
                }
            };

            ("removed-meanwhile", told)
        }
        Asking::PlaceGone(Unplaced::Scene { begins, .. }) => (
            "place-gone",
            say_about("place-gone-scene", "begins", begins.trim()),
        ),
        Asking::PlaceGone(Unplaced::Section { titled }) => (
            "place-gone",
            say_about("place-gone-section", "section", titled.trim()),
        ),
        Asking::RenamedMeanwhile { mine, theirs, .. } => {
            let mut args = FluentArgs::new();
            args.set("theirs", theirs.clone());
            args.set("mine", mine.clone());

            (
                "renamed-meanwhile",
                say_with("renamed-meanwhile-heading-told", &args),
            )
        }
        Asking::RemoveSection(_) => ("asked-remove-section", say("asked-remove-section-told")),
        Asking::RemoveChosen { scenes, sections } => {
            let mut args = FluentArgs::new();
            args.set("scenes", scenes.len());
            args.set("sections", sections.len());
            let counted = match (scenes.is_empty(), sections.is_empty()) {
                (false, true) => "chosen-scenes",
                (true, false) => "chosen-sections",
                _ => "chosen-both",
            };
            let ways = match state.side {
                Side::Book => "chosen-ways-book",
                Side::Loose => "chosen-ways-loose",
            };
            let told = [
                Some(counted),
                (!sections.is_empty()).then_some("chosen-whole"),
                Some(ways),
            ]
            .into_iter()
            .flatten()
            .map(|sentence| say_with(sentence, &args))
            .collect::<Vec<_>>()
            .join(" ");

            ("asked-remove-chosen", told)
        }
    }
}

fn answered(question: Asking, answer: Answer, state: WriteViewState, go_to: impl Fn(&str)) {
    match question {
        Asking::DeleteScene(scene) => match state.side {
            Side::Book => state.structure.delete(scene),
            Side::Loose => state.loose.delete(scene),
        },
        Asking::RemovedMeanwhile { scene, .. } => go_to(&loose_and_at(&scene)),
        Asking::PlaceGone(unplaced) => match answer {
            Answer::PutAtTheEnd => state.beginnings.put_at_the_end(unplaced),
            _ => state.beginnings.discard(unplaced),
        },
        Asking::RenamedMeanwhile {
            section,
            mine,
            theirs,
            kept,
        } => match answer {
            Answer::KeptMine => {
                kept.keep(mine.clone(), theirs);
                state.structure.retitle(section, mine);
            }
            _ => {
                kept.take(theirs.clone());
                if let Ok(Some(shown)) = document().query_selector(&format!(
                    ".heading-row[data-section=\"{section}\"] .heading"
                )) {
                    shown.set_text_content(Some(&theirs));
                }
            }
        },
        Asking::RemoveSection(section) => {
            state
                .loose
                .include(untrack(|| state.structure.scenes_within(&section)));
            state.structure.remove(section);
        }
        Asking::RemoveChosen { scenes, sections } => {
            state.choosing.let_go();
            match (answer, state.side) {
                (Answer::TakenOut, _) => {
                    state.loose.include(scenes.clone());
                    for scene in scenes {
                        state.structure.take_out(scene);
                    }
                }
                (_, Side::Book) => {
                    for scene in scenes {
                        state.structure.delete(scene);
                    }
                }
                (_, Side::Loose) => {
                    for scene in scenes {
                        state.loose.delete(scene);
                    }
                }
            }
            for section in sections.into_iter().rev() {
                state.structure.remove(section);
            }
        }
    }
}
