use leptos::IntoView;
use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use crate::passages::editor::{PassageEditor, PassageEditorProps};
use crate::passages::model::PassageId;
use crate::shell::{Inside, masthead};

#[component]
pub fn OnePassage() -> impl IntoView {
    let params = use_params_map();
    let whose = move || params.read().get("project").unwrap_or_default();

    (
        move || masthead(Some(Inside::of(&whose(), None))),
        html::main().child(html::div().class("column").child(move || {
            match params.read().get("passage") {
                Some(passage) => PassageEditor(PassageEditorProps {
                    passage: PassageId::from(passage),
                })
                .into_any(),
                None => html::p().class("empty").child("Opening…").into_any(),
            }
        })),
    )
}
