use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::http::ApiError;
use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passages;
use crate::shell::{Inside, masthead};

#[component]
pub fn OnePassage() -> impl IntoView {
    let params = use_params_map();
    let problem = RwSignal::new(None::<ApiError>);
    let passage = RwSignal::new(None::<PassageId>);

    let opening = Action::new_local(move |asked: &PassageId| {
        let asked = asked.clone();

        async move {
            match passages::open(&asked).await {
                Ok(found) => {
                    problem.set(None);
                    passage.set(Some(found));
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
            move || match passage.get() {
                Some(passage) => PassageEditor(PassageEditorProps { passage }).into_any(),
                None => html::p().class("empty").child("Opening…").into_any(),
            },
        ))),
    )
}
