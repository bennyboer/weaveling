use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::route;
use crate::shell::masthead;
use crate::speech::said;

#[component]
pub fn MissingPage() -> impl IntoView {
    (
        masthead(None),
        html::main().child(html::div().class("column").child((
            html::p().class("empty").child(said("missing")),
            view! {
                <A href=route::WORKSPACE attr:class="back">
                    {said("missing-back")}
                </A>
            },
        ))),
    )
}
