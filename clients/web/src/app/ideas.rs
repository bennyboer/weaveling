use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;

use crate::app::project_page;
use crate::ideas::pool::{Pool, PoolProps};
use crate::shell::Viewing;

#[component]
pub fn IdeasPage() -> impl IntoView {
    project_page::project_page(Viewing::Ideas, |project| {
        html::div()
            .class("column")
            .child(Pool(PoolProps { project }))
    })
}
