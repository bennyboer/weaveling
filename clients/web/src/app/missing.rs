use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, view};
use leptos_router::components::A;

use crate::route;
use crate::shell::masthead;

#[component]
pub fn Missing() -> impl IntoView {
    (
        masthead(None),
        html::main().child(
            html::div().class("column").child((
                html::p()
                    .class("empty")
                    .child("There is nothing woven at this address."),
                view! {
                    <A href=route::WORKSPACE attr:class="back">
                        "Back to your projects"
                    </A>
                },
            )),
        ),
    )
}
