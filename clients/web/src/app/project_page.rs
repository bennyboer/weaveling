use leptos::prelude::*;
use leptos::{IntoView, html};
use leptos_router::hooks;

use crate::shell;
use crate::shell::{Inside, Viewing};

pub fn project_page<V: IntoView + 'static>(
    viewing: Viewing,
    shown: impl Fn(String) -> V + Send + Sync + 'static,
) -> impl IntoView {
    let params = hooks::use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                shell::masthead(Some(Inside::of(&project, Some(viewing)))),
                html::main().child(shown(project)),
            )
        })
    }
}
