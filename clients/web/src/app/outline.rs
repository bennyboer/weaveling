use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::outline::tree::{TheOutline, TheOutlineProps};
use crate::shell::{Inside, Viewing, masthead};

#[component]
pub fn OneOutline() -> impl IntoView {
    let params = use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                masthead(Some(Inside::of(&project, Some(Viewing::Outline)))),
                html::main().child(TheOutline(TheOutlineProps { project })),
            )
        })
    }
}
