use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::ideas::inspector::inspector;
use crate::ideas::inspector_state::InspectorState;
use crate::route;
use crate::shell::{Inside, masthead};

#[component]
pub fn OneIdea() -> impl IntoView {
    let params = use_params_map();
    let project = Memo::new(move |_| params.read().get("project").unwrap_or_default());
    let asked = Signal::derive(move || {
        params
            .read()
            .get("idea")
            .map(|segment| route::idea_id(&segment))
    });
    let state = InspectorState::inspecting(project, asked);
    let ready = Memo::new(move |_| state.opened().is_some());

    (
        move || masthead(Some(Inside::of(&state.project(), None))),
        html::main().child(html::div().class("column").child((
            move || {
                state.problem().map(|failure| {
                    html::p()
                        .class("problem")
                        .role("alert")
                        .child(failure.to_string())
                })
            },
            move || match ready.get() {
                true => inspector(state).into_any(),
                false => html::p().class("empty").child("Opening…").into_any(),
            },
        ))),
    )
}
