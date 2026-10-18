use leptos::prelude::*;
use leptos::{IntoView, html};
use leptos_router::hooks::use_params_map;

use crate::shell::{Inside, Viewing, masthead};

pub fn project_page<V: IntoView + 'static>(
    viewing: Viewing,
    shown: impl Fn(String) -> V + Send + Sync + 'static,
) -> impl IntoView {
    let params = use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                masthead(Some(Inside::of(&project, Some(viewing)))),
                html::main().child(shown(project)),
            )
        })
    }
}
