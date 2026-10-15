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
}

pub fn asking(state: WriteViewState) -> impl IntoView {
    move || {
        state.margin.asking().map(|question| {
            let (title, told, confirm) = worded(&question, state);
            let destroys = matches!(question, Asking::DeleteScene(_));

            html::div()
                .class("overlay")
                .on(ev::click, move |_| state.margin.return_to_writing())
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
                                    .on(ev::click, move |_| state.margin.return_to_writing())
                                    .child("Cancel"),
                                html::button()
                                    .class(("danger", destroys))
                                    .on(ev::click, move |_| {
                                        state.margin.return_to_writing();
                                        answered(question.clone(), state);
                                    })
                                    .child(confirm),
                            )),
                        )),
                )
        })
    }
}

fn worded(question: &Asking, state: WriteViewState) -> (&'static str, String, &'static str) {
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
                "Delete",
            )
        }
        Asking::RemoveSection(_) => (
            "Remove this section?",
            "Its scenes leave the book but are not deleted: they wait among the scenes not in the book. Sections inside it move up to take its place.".to_owned(),
            "Remove",
        ),
    }
}

fn answered(question: Asking, state: WriteViewState) {
    match question {
        Asking::DeleteScene(scene) => match state.side {
            Side::Book => state.structure.delete(scene),
            Side::Loose => state.loose.delete(scene),
        },
        Asking::RemoveSection(section) => state.structure.remove(section),
    }
}
