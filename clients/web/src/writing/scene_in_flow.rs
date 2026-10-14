use leptos::IntoView;
use leptos::html;
use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::closure::Closure;
use words::words_in;

use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::writing::viewport::SCENE_PLACE;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

const AUTHOR: &str = "You";
const COLOUR: &str = "#f59e0b";

const WORDS_A_LINE: usize = 11;

pub fn scene_in_flow(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let place: NodeRef<Div> = NodeRef::new();
    let near = Memo::new({
        let scene = scene.clone();

        move |_| state.viewport.is_near(&scene)
    });

    Effect::new(move || {
        if let Some(watched) = place.get() {
            state.viewport.watch(&watched);
        }
    });
    on_cleanup(move || {
        if let Some(watched) = place.get_untracked() {
            state.viewport.stop_watching(&watched);
        }
    });

    html::div()
        .class("scene-place")
        .attr(SCENE_PLACE, scene.to_string())
        .node_ref(place)
        .child(move || {
            if near.get() {
                editor(scene.clone(), state).into_any()
            } else {
                waiting(scene.clone(), state).into_any()
            }
        })
}

fn waiting(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let lines = untrack(|| state.word_count.of_scene(&scene))
        .div_ceil(WORDS_A_LINE)
        .max(1);

    html::div()
        .class("scene waiting")
        .attr("data-scene", scene.to_string())
        .attr("aria-busy", "true")
        .style(("min-height", format!("{lines}lh")))
}

fn editor(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let kept = StoredValue::new_local(
        None::<(
            Closure<dyn Fn(bool)>,
            Closure<dyn Fn(String) -> bool>,
            Closure<dyn Fn()>,
            Closure<dyn Fn(String)>,
            Closure<dyn Fn(bool)>,
            Closure<dyn Fn(u32)>,
        )>,
    );
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

                    state.walk.leave(&Stop::Scene(scene.clone()), towards)
                }
            });
            editor.on_leaving(&leaving);
            let breaking = Closure::<dyn Fn()>::new({
                let scene = scene.clone();

                move || state.beginnings.start_pending_scene_after(&scene)
            });
            editor.on_breaking(&breaking);
            let changed = Closure::<dyn Fn(String)>::new({
                let scene = scene.clone();

                move |text: String| {
                    state.word_count.scene_has(scene.clone(), words_in(&text));
                    state.labels.opening_is(scene.clone(), text);
                }
            });
            editor.on_changed(&changed);
            let saving = Closure::<dyn Fn(bool)>::new({
                let scene = scene.clone();

                move |saving: bool| state.saving.scene_is_saving(scene.clone(), saving)
            });
            editor.on_saving(&saving);
            let caret_moved = Closure::<dyn Fn(u32)>::new({
                let scene = scene.clone();

                move |at: u32| state.left_off.remember(&scene, at)
            });
            editor.on_caret_moved(&caret_moved);
            if let Some(carried) = state.beginnings.words_to_carry(&scene) {
                editor.seed(&carried);
            }

            state.walk.writing_in(scene.clone(), editor);
            kept.set_value(Some((
                connected,
                leaving,
                breaking,
                changed,
                saving,
                caret_moved,
            )));
        }
    });

    on_cleanup({
        let scene = scene.clone();

        move || {
            if let Some(editor) = state.walk.done_with(&scene) {
                editor.destroy();
            }
            state.saving.let_go_of(&scene);
            kept.set_value(None);
        }
    });

    html::div()
        .class("scene")
        .attr("data-scene", named)
        .node_ref(host)
}
