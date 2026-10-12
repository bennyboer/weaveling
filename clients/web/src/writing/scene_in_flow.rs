use leptos::IntoView;
use leptos::html;
use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::closure::Closure;

use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::writing::write_state::{Stop, Towards, WriteState};

const AUTHOR: &str = "You";
const COLOUR: &str = "#f59e0b";

pub fn scene_in_flow(scene: SceneId, state: WriteState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let kept =
        StoredValue::new_local(None::<(Closure<dyn Fn(bool)>, Closure<dyn Fn(String) -> bool>)>);
    let named = scene.to_string();

    Effect::new({
        let scene = scene.clone();

        move || {
            let Some(surface) = host.get() else {
                return;
            };

            let connected = Closure::<dyn Fn(bool)>::new(|_: bool| {});
            let editor = ProseEditor::new(&surface, scene.as_str(), AUTHOR, COLOUR, &connected);
            let leaving = Closure::<dyn Fn(String) -> bool>::new({
                let scene = scene.clone();

                move |towards: String| {
                    let towards = match towards.as_str() {
                        "before" => Towards::Before,
                        _ => Towards::After,
                    };

                    state.leave(&Stop::Scene(scene.clone()), towards)
                }
            });
            editor.on_leaving(&leaving);

            state.writing_in(scene.clone(), editor);
            kept.set_value(Some((connected, leaving)));
        }
    });

    on_cleanup({
        let scene = scene.clone();

        move || {
            if let Some(editor) = state.done_with(&scene) {
                editor.destroy();
            }
            kept.set_value(None);
        }
    });

    html::div()
        .class("scene")
        .attr("data-scene", named)
        .node_ref(host)
}
