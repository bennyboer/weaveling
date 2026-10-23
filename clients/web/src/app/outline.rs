use leptos::IntoView;
use leptos::prelude::*;

use crate::app::project_page;
use crate::outline::tree::{OutlineView, OutlineViewProps};
use crate::shell::Viewing;

#[component]
pub fn OutlinePage() -> impl IntoView {
    project_page::project_page(Viewing::Outline, |project| {
        OutlineView(OutlineViewProps { project })
    })
}
