use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::shell::{Inside, Viewing, masthead};
use crate::writing::write_view::{WriteView, WriteViewProps};

#[component]
pub fn WritePage() -> impl IntoView {
    let params = use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                masthead(Some(Inside::of(&project, Some(Viewing::Write)))),
                html::main().child(WriteView(WriteViewProps { project })),
            )
        })
    }
}
