use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;

use crate::dialog::modal;
use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
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
                title,
                "",
                cancel,
                (
                    html::p().child(told),
                    html::div().class("dialog-actions").child((
                        untrack(|| question.with_value(way_back)).map(|way_back| {
                            html::button()
                                .on(ev::click, move |_| cancel())
                                .child(way_back)
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
                                    .child(label)
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
        Asking::RemovedMeanwhile { .. } => Some("Keep writing"),
        Asking::PlaceGone(_) => None,
        _ => Some("Cancel"),
    }
}

fn loose_and_at(scene: &SceneId) -> String {
    let path = window().location().pathname().unwrap_or_default();
    let book = path.trim_end_matches('/').trim_end_matches("/loose");

    format!("{book}/loose?scene={scene}")
}

fn offered(question: &Asking, state: WriteViewState) -> Vec<(&'static str, Answer)> {
    match question {
        Asking::DeleteScene(_) => vec![("Delete", Answer::DeletedForGood)],
        Asking::RemoveSection(_) => vec![("Remove", Answer::Confirmed)],
        Asking::RemovedMeanwhile { .. } => vec![("Show it", Answer::ShownElsewhere)],
        Asking::PlaceGone(_) => vec![
            ("Discard", Answer::Discarded),
            ("Put it at the end of the book", Answer::PutAtTheEnd),
        ],
        Asking::RenamedMeanwhile { .. } => vec![
            ("Take theirs", Answer::TakenTheirs),
            ("Keep mine", Answer::KeptMine),
        ],
        Asking::RemoveChosen { .. } => match state.side {
            Side::Book => vec![
                ("Take out of the book", Answer::TakenOut),
                ("Delete for good", Answer::DeletedForGood),
            ],
            Side::Loose => vec![("Delete for good", Answer::DeletedForGood)],
        },
    }
}

fn worded(question: &Asking, state: WriteViewState) -> (&'static str, String) {
    match question {
        Asking::DeleteScene(scene) => {
            let shown = untrack(|| state.labels.of_scene(scene)).shown;
            let named = if shown.trim().is_empty() {
                "This scene".to_owned()
            } else {
                format!("“{shown}”")
            };

            (
                "Delete this scene?",
                format!("{named} and everything written in it will be gone for good. Deleting cannot be undone."),
            )
        }
        Asking::RemovedMeanwhile { title, scene } => {
            let shown = untrack(|| state.labels.of_scene(scene)).shown;
            let named = match shown.trim() {
                "" => "Your scene".to_owned(),
                shown => format!("Your scene “{shown}”"),
            };

            (
                "Removed meanwhile",
                format!(
                    "Someone else removed “{}”. {named} is outside the book now, among the scenes not in it.",
                    title.trim()
                ),
            )
        }
        Asking::PlaceGone(Unplaced::Scene { begins, .. }) => (
            "Its place is gone",
            format!(
                "Someone else removed where your new scene was going. It begins “{}”.",
                begins.trim()
            ),
        ),
        Asking::PlaceGone(Unplaced::Section { titled }) => (
            "Its place is gone",
            format!(
                "Someone else removed what your new section “{}” was going after.",
                titled.trim()
            ),
        ),
        Asking::RenamedMeanwhile { mine, theirs, .. } => (
            "Renamed meanwhile",
            format!(
                "While you were editing this heading, someone else renamed it to “{theirs}”. Yours is “{mine}”."
            ),
        ),
        Asking::RemoveSection(_) => (
            "Remove this section?",
            "Its scenes leave the book but are not deleted: they wait among the scenes not in the book. Sections inside it move up to take its place.".to_owned(),
        ),
        Asking::RemoveChosen { scenes, sections } => {
            let counted = match (scenes.len(), sections.len()) {
                (scenes, 0) => counting(scenes, "scene"),
                (0, sections) => counting(sections, "section"),
                (scenes, sections) => format!(
                    "{} and {}",
                    counting(scenes, "scene"),
                    counting(sections, "section")
                ),
            };
            let whole = match sections.is_empty() {
                true => "",
                false => " Each section goes with the sections inside it and all their scenes.",
            };
            let ways = match state.side {
                Side::Book => " Taken out of the book, the scenes wait among those not in it. Deleted for good, they and everything written in them are gone, and that cannot be undone.",
                Side::Loose => " Deleted for good, they and everything written in them are gone, and that cannot be undone.",
            };

            let are = match scenes.len() + sections.len() {
                1 => "is",
                _ => "are",
            };

            (
                "Remove what is chosen?",
                format!("{counted} {are} chosen.{whole}{ways}"),
            )
        }
    }
}

fn counting(count: usize, thing: &str) -> String {
    match count {
        1 => format!("1 {thing}"),
        _ => format!("{count} {thing}s"),
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
