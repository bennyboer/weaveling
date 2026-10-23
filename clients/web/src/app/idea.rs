use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks;

use crate::ideas::inspector;
use crate::ideas::inspector_state::InspectorState;
use crate::problem;
use crate::route;
use crate::shell;
use crate::shell::Inside;
use crate::speech;

#[component]
pub fn IdeaPage() -> impl IntoView {
    let params = hooks::use_params_map();
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
        move || shell::masthead(Some(Inside::of(&state.project(), None))),
        html::main().child(html::div().class("column").child((
            move || state.problem().map(problem::problem),
            move || {
                match ready.get() {
                    true => inspector::inspector(state).into_any(),
                    false => html::p()
                        .class("empty")
                        .child(speech::said("idea-opening"))
                        .into_any(),
                }
            },
        ))),
    )
}
