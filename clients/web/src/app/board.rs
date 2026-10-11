use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::boards::board::{BoardView, BoardViewProps};
use crate::shell::{Inside, Viewing, masthead};

#[component]
pub fn BoardPage() -> impl IntoView {
    let params = use_params_map();

    move || {
        params.read().get("project").map(|project| {
            (
                masthead(Some(Inside::of(&project, Some(Viewing::Board)))),
                html::main().child(BoardView(BoardViewProps { project })),
            )
        })
    }
}
