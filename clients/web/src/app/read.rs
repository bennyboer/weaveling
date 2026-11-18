use leptos::IntoView;
use leptos::prelude::*;
use leptos_router::hooks;

use crate::app::project_page;
use crate::readings::view;
use crate::shell::Viewing;

#[component]
pub fn ReadPage() -> impl IntoView {
    let params = hooks::use_params_map();

    project_page::project_page(Viewing::Read, move |project| {
        view::reading_view(project, params.read().get("copy"))
    })
}
