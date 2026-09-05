use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::pieces::pool::{Pool, PoolProps};
use crate::shell::{Inside, Viewing, masthead};

#[component]
pub fn OnePool() -> impl IntoView {
    let params = use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                masthead(Some(Inside::of(&project, Some(Viewing::Pieces)))),
                html::main().child(
                    html::div()
                        .class("column")
                        .child(Pool(PoolProps { project })),
                ),
            )
        })
    }
}
