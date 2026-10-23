use fluent_bundle::FluentArgs;
use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::projects::languages::BOOK_LANGUAGES;
use crate::projects::model::ProjectDetails;
use crate::speech;
use crate::speech::Speech;
use quotation::QuotationMarks;

#[derive(Clone, Copy)]
pub struct DetailsDraft {
    pub title: RwSignal<String>,
    pub language: RwSignal<String>,
    pub quotation: RwSignal<String>,
    pub description: RwSignal<String>,
}

impl DetailsDraft {
    pub fn of(title: RwSignal<String>, details: &ProjectDetails) -> Self {
        Self {
            title,
            language: RwSignal::new(details.language.clone().unwrap_or_default()),
            quotation: RwSignal::new(
                details
                    .quotation
                    .map(|style| style.name().to_owned())
                    .unwrap_or_default(),
            ),
            description: RwSignal::new(details.description.clone()),
        }
    }

    pub fn details(self) -> ProjectDetails {
        ProjectDetails {
            language: Some(self.language.get_untracked()).filter(|chosen| !chosen.is_empty()),
            quotation: QuotationMarks::named(&self.quotation.get_untracked()),
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
    let speech = expect_context::<Speech>();

    Effect::new(move || {
        if let Some(field) = title.get() {
            let _ = field.focus();
            field.select();
        }
    });

    (
        html::label().class("field").child((
            html::span().child(speech::said("details-title")),
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
            html::span().child(speech::said("details-book-language")),
            html::select()
                .on(ev::change, move |event| {
                    draft.language.set(event_target_value(&event))
                })
                .child((
                    html::option()
                        .value("")
                        .prop("selected", move || draft.language.get().is_empty())
                        .child(speech::said("details-book-language-unset")),
                    BOOK_LANGUAGES
                        .iter()
                        .map(|tag| {
                            html::option()
                                .value(*tag)
                                .prop("selected", move || draft.language.get() == *tag)
                                .child(move || speech.say(&format!("book-language-{tag}")))
                        })
                        .collect::<Vec<_>>(),
                )),
            html::small().child(speech::said("details-book-language-hint")),
        )),
        html::label().class("field").child((
            html::span().child(speech::said("details-quotation")),
            html::select()
                .on(ev::change, move |event| {
                    draft.quotation.set(event_target_value(&event))
                })
                .child((
                    html::option()
                        .value("")
                        .prop("selected", move || draft.quotation.get().is_empty())
                        .child(move || {
                            let language = draft.language.get();
                            let chosen = Some(language.as_str()).filter(|tag| !tag.is_empty());

                            let mut args = FluentArgs::new();
                            args.set("marks", QuotationMarks::of_language(chosen).shown());

                            speech.say_with("details-quotation-as-the-language-has-it", &args)
                        }),
                    QuotationMarks::ALL
                        .iter()
                        .map(|style| {
                            let style = *style;

                            html::option()
                                .value(style.name())
                                .prop("selected", move || draft.quotation.get() == style.name())
                                .child(style.shown())
                        })
                        .collect::<Vec<_>>(),
                )),
            html::small().child(speech::said("details-quotation-hint")),
        )),
        html::label().class("field").child((
            html::span().child(speech::said("details-description")),
            html::textarea()
                .attr("rows", "3")
                .attr(
                    "placeholder",
                    speech::said("details-description-placeholder"),
                )
                .prop("value", move || draft.description.get())
                .on(ev::input, move |event| {
                    draft.description.set(event_target_value(&event))
                }),
        )),
    )
}
