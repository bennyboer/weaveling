use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::route;
use crate::shell;
use crate::speech;

#[component]
pub fn MissingPage() -> impl IntoView {
    (
        shell::masthead(None),
        html::main().child(html::div().class("column").child((
            html::p().class("empty").child(speech::said("missing")),
            view! {
                <A href=route::WORKSPACE attr:class="back">
                    {speech::said("missing-back")}
                </A>
            },
        ))),
    )
}
