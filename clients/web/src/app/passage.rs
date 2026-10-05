use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::passages::drawn_from::drawn_from;
use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::link_dialog::link_dialog;
use crate::passages::open_passage::OpenPassage;
use crate::passages::title_field::title_field;
use crate::shell::{Inside, masthead};

#[component]
pub fn OnePassage() -> impl IntoView {
    let params = use_params_map();
    let project = Memo::new(move |_| params.read().get("project").unwrap_or_default());
    let asked = Memo::new(move |_| params.read().get("passage"));
    let open = OpenPassage::open(project, asked);

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
            move || {
                match open.opened() {
                    Some(passage) => (
                        PassageEditor(PassageEditorProps { passage }),
                        title_field(open),
                        drawn_from(open),
                        move || open.is_picking().then(|| link_dialog(open)),
                    )
                        .into_any(),
                    None => html::p().class("empty").child("Opening…").into_any(),
                }
            },
        ))),
    )
}
