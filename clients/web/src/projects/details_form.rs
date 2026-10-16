use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::projects::languages::BOOK_LANGUAGES;
use crate::projects::model::ProjectDetails;

#[derive(Clone, Copy)]
pub struct DetailsDraft {
    pub title: RwSignal<String>,
    pub language: RwSignal<String>,
    pub description: RwSignal<String>,
}

impl DetailsDraft {
    pub fn of(title: RwSignal<String>, details: &ProjectDetails) -> Self {
        Self {
            title,
            language: RwSignal::new(details.language.clone().unwrap_or_default()),
            description: RwSignal::new(details.description.clone()),
        }
    }

    pub fn details(self) -> ProjectDetails {
        ProjectDetails {
            language: Some(self.language.get_untracked()).filter(|chosen| !chosen.is_empty()),
            description: self.description.get_untracked(),
        }
    }
}

pub fn details_fields(
    draft: DetailsDraft,
    title: NodeRef<Input>,
    edited: impl Fn() + Copy + Send + Sync + 'static,
    submit: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    Effect::new(move || {
        if let Some(field) = title.get() {
            let _ = field.focus();
            field.select();
        }
    });

    (
        html::label().class("field").child((
            html::span().child("Title"),
            html::input()
                .r#type("text")
                .node_ref(title)
                .prop("value", move || draft.title.get())
                .on(ev::input, move |event| {
                    edited();
                    draft.title.set(event_target_value(&event));
                })
                .on(ev::keydown, move |event| {
                    if event.key() == "Enter" {
                        submit();
                    }
                }),
        )),
        html::label().class("field").child((
            html::span().child("Language of the book"),
            html::select()
                .on(ev::change, move |event| {
                    draft.language.set(event_target_value(&event))
                })
                .child((
                    html::option()
                        .value("")
                        .prop("selected", move || draft.language.get().is_empty())
                        .child("Not set"),
                    BOOK_LANGUAGES
                        .iter()
                        .map(|(tag, named)| {
                            html::option()
                                .value(*tag)
                                .prop("selected", move || draft.language.get() == *tag)
                                .child(*named)
                        })
                        .collect::<Vec<_>>(),
                )),
            html::small().child(
                "The language the book is written in, used wherever its text depends on it.",
            ),
        )),
        html::label().class("field").child((
            html::span().child("Description"),
            html::textarea()
                .attr("rows", "3")
                .attr(
                    "placeholder",
                    "What the book is about, in a few lines. Optional.",
                )
                .prop("value", move || draft.description.get())
                .on(ev::input, move |event| {
                    draft.description.set(event_target_value(&event))
                }),
        )),
    )
}
