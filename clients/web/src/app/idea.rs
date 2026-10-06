use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::ideas::inspector::inspector;
use crate::ideas::open_idea::OpenIdea;
use crate::shell::{Inside, masthead};

#[component]
pub fn OneIdea() -> impl IntoView {
    let params = use_params_map();
    let project = Memo::new(move |_| params.read().get("project").unwrap_or_default());
    let asked = Memo::new(move |_| params.read().get("idea"));
    let open = OpenIdea::open(project, asked);
    let ready = Memo::new(move |_| open.opened().is_some());

    (
        move || masthead(Some(Inside::of(&open.project(), None))),
        html::main().child(html::div().class("column").child((
            move || {
                open.problem().map(|failure| {
                    html::p()
                        .class("problem")
                        .role("alert")
                        .child(failure.to_string())
                })
            },
            move || match ready.get() {
                true => inspector(open).into_any(),
                false => html::p().class("empty").child("Opening…").into_any(),
            },
        ))),
    )
}
