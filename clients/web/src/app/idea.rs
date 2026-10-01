use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::http::ApiError;
use crate::ideas::model::IdeaId;
use crate::ideas::service as ideas;
use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::passages::service as passages;
use crate::route;
use crate::shell::{Inside, masthead};

#[component]
pub fn OneIdea() -> impl IntoView {
    let params = use_params_map();
    let problem = RwSignal::new(None::<ApiError>);
    let passage = RwSignal::new(None::<PassageId>);

    let opening = Action::new_local(move |idea: &IdeaId| {
        let idea = idea.clone();

        async move {
            match writing_in(&idea).await {
                Ok(found) => {
                    problem.set(None);
                    passage.set(Some(found));
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    Effect::new(move || {
        if let Some(idea) = params.read().get("idea") {
            opening.dispatch(route::idea_id(&idea));
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
                match passage.get() {
                    Some(passage) => PassageEditor(PassageEditorProps { passage }).into_any(),
                    None => html::p()
                        .class("empty")
                        .child("Opening the idea…")
                        .into_any(),
                }
            },
        ))),
    )
}

async fn writing_in(idea: &IdeaId) -> Result<PassageId, ApiError> {
    let found = ideas::get(idea).await?;

    match found.passage {
        Some(passage) => Ok(passage),
        None => {
            let started = passages::create().await?;

            ideas::attach_passage(idea, &started)
                .await?
                .passage
                .ok_or(ApiError::Unexpected)
        }
    }
}
