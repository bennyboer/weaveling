use leptos::prelude::*;
use leptos::{IntoView, html};
use leptos_router::hooks;
use projects_contract::refused::ID_INVALID;

use crate::app::missing;
use crate::http::ApiError;
use crate::projects::open_project::OpenProject;
use crate::route;
use crate::shell;
use crate::shell::{Inside, Viewing};
use crate::speech;

/// A view of a project, under its masthead; or, once the project is found not to be there, a page saying so.
pub fn project_page<V: IntoView + 'static>(
    viewing: Viewing,
    shown: impl Fn(String) -> V + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let params = hooks::use_params_map();

    move || {
        params.read().get("project").map(|project| {
            let shown = shown.clone();
            let inside = project.clone();

            unless_gone(&project, move || {
                (
                    shell::masthead(Some(Inside::of(&inside, Some(viewing)))),
                    html::main().child(shown(inside.clone())),
                )
            })
        })
    }
}

/// A page of a project while the project is there; once it is found not to be, a page saying so instead,
/// rather than the project's masthead over a page that has nothing to show.
pub fn unless_gone<V: IntoView + 'static, P: Fn() -> V + Send + Sync + 'static>(
    project: &str,
    page: P,
) -> impl IntoView + use<V, P> {
    let open_project = expect_context::<OpenProject>();
    open_project.open(&route::project_id(project));
    let gone = Memo::new(move |_| open_project.refused().as_ref().and_then(gone_as));

    move || match gone.get() {
        Some(Gone::NoMore) => {
            missing::not_found(|| speech::say_about("problem-not-found", "subject", "project"))
                .into_any()
        }
        Some(Gone::NeverThere) => missing::not_found(|| speech::say("missing")).into_any(),
        None => page().into_any(),
    }
}

/// Why a project is not there: it was, and is no more; or its address names none, as its id is not even one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Gone {
    NoMore,
    NeverThere,
}

fn gone_as(refused: &ApiError) -> Option<Gone> {
    match refused {
        ApiError::NotFound(_) => Some(Gone::NoMore),
        ApiError::Rejected(refusal) if refusal.kind.as_deref() == Some(ID_INVALID) => {
            Some(Gone::NeverThere)
        }
        _ => None,
    }
}
