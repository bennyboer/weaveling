use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::http::ApiError;
use crate::ideas::model::Idea;
use crate::ideas::service as ideas;
use crate::route;
use crate::shell::{Inside, masthead};

#[component]
pub fn OneIdea() -> impl IntoView {
    let params = use_params_map();
    let problem = RwSignal::new(None::<ApiError>);
    let idea = RwSignal::new(None::<Idea>);

    let opening = Action::new_local(move |asked: &String| {
        let asked = route::idea_id(asked);

        async move {
            match ideas::get(&asked).await {
                Ok(found) => {
                    problem.set(None);
                    idea.set(Some(found));
                }
                Err(failure) => problem.set(Some(failure)),
            }
        }
    });

    Effect::new(move || {
        if let Some(asked) = params.read().get("idea") {
            opening.dispatch(asked);
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
            move || match idea.get() {
                // TODO M12 step 8: the backlinks, and the name editable in place.
                Some(found) => html::h1().child(found.shown_as().to_owned()).into_any(),
                None => html::p().class("empty").child("Opening…").into_any(),
            },
        ))),
    )
}
