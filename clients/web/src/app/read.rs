use leptos::IntoView;
use leptos::prelude::*;

use crate::app::project_page;
use crate::readings::view;
use crate::shell::Viewing;

#[component]
pub fn ReadPage() -> impl IntoView {
    project_page::project_page(Viewing::Read, view::reading_view)
}
