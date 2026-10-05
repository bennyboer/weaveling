use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};
use leptos_router::hooks::use_params_map;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::http::ApiError;
use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passages;
use crate::shell::{Inside, masthead};

#[component]
pub fn OnePassage() -> impl IntoView {
    let params = use_params_map();
    let problem = RwSignal::new(None::<ApiError>);
    let opened = RwSignal::new(None::<PassageId>);
    let title = RwSignal::new(String::new());

    let opening = Action::new_local(move |asked: &PassageId| {
        let asked = asked.clone();

        async move {
            match passages::open(&asked).await {
                Ok(found) => {
                    problem.set(None);
                    title.set(found.title);
                    opened.set(Some(found.id));
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    let retitling = Action::new_local(move |(passage, typed): &(PassageId, String)| {
        let passage = passage.clone();
        let typed = typed.clone();

        async move {
            match passages::retitle(&passage, &typed).await {
                Ok(retitled) => {
                    problem.set(None);
                    title.set(retitled.title);
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    Effect::new(move || {
        if let Some(asked) = params.read().get("passage") {
            opening.dispatch(PassageId::from(asked));
        }
    });

    let whose = move || params.read().get("project").unwrap_or_default();

    (
        move || masthead(Some(Inside::of(&whose(), None))),
        html::main().child(html::div().class("column").child((
            move || {
                problem.get().map(|failure| {
                    html::p()
                        .class("problem")
                        .role("alert")
                        .child(failure.to_string())
                })
            },
            move || {
                match opened.get() {
                    Some(passage) => (
                        PassageEditor(PassageEditorProps {
                            passage: passage.clone(),
                        }),
                        about(passage, title, retitling),
                    )
                        .into_any(),
                    None => html::p().class("empty").child("Opening…").into_any(),
                }
            },
        ))),
    )
}

fn about(
    passage: PassageId,
    title: RwSignal<String>,
    retitling: Action<(PassageId, String), ()>,
) -> impl IntoView {
    html::section().class("about").child(
        html::input()
            .r#type("text")
            .class("passage-title")
            .attr("aria-label", "Passage title")
            .attr(
                "placeholder",
                "Untitled \u{2014} the outline shows its opening words",
            )
            .prop("value", move || title.get())
            .on(ev::change, move |event| {
                let Some(field) = event
                    .target()
                    .and_then(|it| it.dyn_into::<HtmlInputElement>().ok())
                else {
                    return;
                };

                retitling.dispatch((passage.clone(), field.value()));
            }),
    )
}
