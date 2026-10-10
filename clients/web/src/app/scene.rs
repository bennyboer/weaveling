use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::scenes::drawn_from::drawn_from;
use crate::scenes::editor::{SceneEditor, SceneEditorProps};
use crate::scenes::link_dialog::link_dialog;
use crate::scenes::scene_page_state::ScenePageState;
use crate::scenes::title_field::title_field;
use crate::shell::{Inside, masthead};

#[component]
pub fn OneScene() -> impl IntoView {
    let params = use_params_map();
    let project = Memo::new(move |_| params.read().get("project").unwrap_or_default());
    let asked = Memo::new(move |_| params.read().get("scene"));
    let state = ScenePageState::open(project, asked);

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
            move || {
                match state.opened() {
                    Some(scene) => (
                        SceneEditor(SceneEditorProps { scene }),
                        title_field(state),
                        drawn_from(state),
                        move || state.is_picking().then(|| link_dialog(state)),
                    )
                        .into_any(),
                    None => html::p().class("empty").child("Opening…").into_any(),
                }
            },
        ))),
    )
}
