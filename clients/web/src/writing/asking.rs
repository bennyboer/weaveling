use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Confirmed,
    TakenOut,
    DeletedForGood,
}

pub fn asking(state: WriteViewState) -> impl IntoView {
    move || {
        state.margin.asking().map(|question| {
            let (title, told) = worded(&question, state);
            let question = StoredValue::new(question);
            let choices = untrack(|| question.with_value(|question| offered(question, state)));
            let cancel = move || state.margin.return_to_writing();

            html::div()
                .class("overlay")
                .on(ev::click, move |_| cancel())
                .child(
                    html::div()
                        .class("dialog")
                        .role("dialog")
                        .attr("aria-modal", "true")
                        .attr("aria-label", title)
                        .on(ev::click, |event| event.stop_propagation())
                        .child((
                            html::h2().child(title),
                            html::p().child(told),
                            html::div().class("dialog-actions").child((
                                html::button()
                                    .on(ev::click, move |_| cancel())
                                    .child("Cancel"),
                                choices
                                    .into_iter()
                                    .map(|(label, answer)| {
                                        html::button()
                                            .class(("danger", answer == Answer::DeletedForGood))
                                            .on(ev::click, move |_| {
                                                state.margin.return_to_writing();
                                                answered(question.get_value(), answer, state);
                                            })
                                            .child(label)
                                    })
                                    .collect::<Vec<_>>(),
                            )),
                        )),
                )
        })
    }
}

fn offered(question: &Asking, state: WriteViewState) -> Vec<(&'static str, Answer)> {
    match question {
        Asking::DeleteScene(_) => vec![("Delete", Answer::DeletedForGood)],
        Asking::RemoveSection(_) => vec![("Remove", Answer::Confirmed)],
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

fn answered(question: Asking, answer: Answer, state: WriteViewState) {
    match question {
        Asking::DeleteScene(scene) => match state.side {
            Side::Book => state.structure.delete(scene),
            Side::Loose => state.loose.delete(scene),
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
