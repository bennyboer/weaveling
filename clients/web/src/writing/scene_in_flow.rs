use leptos::IntoView;
use leptos::html;
use leptos::html::Div;
use leptos::prelude::*;
use wasm_bindgen::closure::Closure;

use crate::projects::open_project::OpenProject;
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::speech;
use crate::speech::Speech;
use crate::writing::beginnings::CarriedImage;
use crate::writing::choosing;
use crate::writing::formatting;
use crate::writing::images;
use crate::writing::side::Side;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

const WORDS_A_LINE: usize = 11;

const AROUND_A_SCENE: &str = "4rem";

pub fn waiting_scene(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let height = match state.viewport.kept_height(&scene) {
        Some(measured) => format!("{measured}px"),
        None => {
            let lines = untrack(|| state.word_count.of_scene(&scene))
                .div_ceil(WORDS_A_LINE)
                .max(1);

            format!("calc({lines}lh + {AROUND_A_SCENE})")
        }
    };

    html::div()
        .class("scene waiting")
        .attr("data-scene", scene.to_string())
        .attr("aria-busy", "true")
        .style(("min-height", height))
}

pub fn scene_in_flow(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let open_project = expect_context::<OpenProject>();
    let speaking = expect_context::<Speech>();
    let marks = Memo::new(move |_| open_project.quotation_marks());
    let editors_made = RwSignal::new(0_u32);
    let kept = StoredValue::new_local(
        None::<(
            Closure<dyn Fn(bool)>,
            Closure<dyn Fn(String) -> bool>,
            Closure<dyn Fn()>,
            Closure<dyn Fn(String)>,
            Closure<dyn Fn(bool)>,
            Closure<dyn Fn(u32)>,
            Closure<dyn Fn(String)>,
            Closure<dyn Fn(String)>,
            Closure<dyn Fn()>,
            Closure<dyn Fn()>,
            Closure<dyn Fn(js_sys::Array, u32)>,
            Closure<dyn Fn(String) -> bool>,
        )>,
    );
    let named = scene.to_string();
    let account = state.presence.own();

    Effect::new({
        let scene = scene.clone();

        move || {
            let Some(surface) = host.get() else {
                return;
            };

            let connected = Closure::<dyn Fn(bool)>::new(|_: bool| {});
            let editor = state.connection.with_value(|connection| {
                ProseEditor::new(&surface, scene.as_str(), &account, &connected, connection)
            });
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
            let selecting = Closure::<dyn Fn(String) -> bool>::new({
                let scene = scene.clone();

                move |taking: String| {
                    let choosing = state.choosing;
                    match taking.as_str() {
                        "all" => choosing.take_in_everything(),
                        "before" => choosing.begin_at(Stop::Scene(scene.clone())),
                        "after" => choosing.begin_at(Stop::Scene(scene.clone())),
                        "beyond-before" => {
                            choosing.begin_at(Stop::Scene(scene.clone()));
                            choosing.reach(Towards::Before);
                        }
                        _ => {
                            choosing.begin_at(Stop::Scene(scene.clone()));
                            choosing.reach(Towards::After);
                        }
                    }
                    choosing::hand_the_keys_to_the_book();

                    true
                }
            });
            editor.on_selecting(&selecting);
            let breaking = Closure::<dyn Fn()>::new({
                let scene = scene.clone();

                move || state.beginnings.start_pending_scene_after(&scene)
            });
            if state.side == Side::Book {
                editor.on_breaking(&breaking);
            }
            let changed = Closure::<dyn Fn(String)>::new({
                let scene = scene.clone();

                move |text: String| {
                    state
                        .word_count
                        .scene_has(scene.clone(), words::words_in(&text));
                    state.labels.opening_is(scene.clone(), text);
                }
            });
            editor.on_changed(&changed);
            let saving = Closure::<dyn Fn(bool)>::new({
                let scene = scene.clone();

                move |saving: bool| {
                    state.saving.scene_is_saving(scene.clone(), saving);
                    if !saving {
                        state.viewport.saved(&scene);
                    }
                }
            });
            editor.on_saving(&saving);
            let caret_moved = Closure::<dyn Fn(u32)>::new({
                let scene = scene.clone();

                move |at: u32| state.left_off.remember(&scene, at)
            });
            editor.on_caret_moved(&caret_moved);
            let present = Closure::<dyn Fn(String)>::new({
                let scene = scene.clone();

                move |told: String| {
                    let accounts: Vec<String> = serde_json::from_str(&told).unwrap_or_default();
                    state.presence.told(&scene, accounts);
                }
            });
            editor.on_present(&present);
            let selection_shown = Closure::<dyn Fn(String)>::new({
                let scene = scene.clone();

                move |told: String| state.formatting.told(&scene, &told)
            });
            editor.on_selection_shown(&selection_shown);
            let to_toolbar = Closure::<dyn Fn()>::new({
                let scene = scene.clone();

                move || formatting::reached(&scene)
            });
            editor.on_to_toolbar(&to_toolbar);
            let link_asked = Closure::<dyn Fn()>::new({
                let scene = scene.clone();

                move || state.formatting.link_asked(&scene)
            });
            editor.on_link_asked(&link_asked);
            let images_given = Closure::<dyn Fn(js_sys::Array, u32)>::new({
                let scene = scene.clone();
                let speech = expect_context::<Speech>();

                move |files: js_sys::Array, ticket: u32| {
                    images::given_by_editor(scene.clone(), state, speech, files, ticket)
                }
            });
            editor.on_images_given(&images_given);
            if let Some(carried) = state.beginnings.carried(&scene) {
                if let Some(words) = carried.words.filter(|words| !words.is_empty()) {
                    editor.seed(&words);
                }
                let with_images = !carried.images.is_empty();
                for image in carried.images {
                    let ticket = editor.asked_for_image();
                    match image {
                        CarriedImage::Files(files) => {
                            images::given(scene.clone(), state, speaking, files, ticket);
                        }
                        CarriedImage::Asset(asset) => {
                            editor.insert_figure(&asset, ticket);
                            editor.forget_waiting(ticket);
                        }
                    }
                }
                if with_images {
                    editor.focus();
                }
            }

            state.walk.editor_ready(&scene, &editor);
            state.editors.arrived(scene.clone(), editor);
            editors_made.update(|made| *made += 1);
            kept.set_value(Some((
                connected,
                leaving,
                breaking,
                changed,
                saving,
                caret_moved,
                present,
                selection_shown,
                to_toolbar,
                link_asked,
                images_given,
                selecting,
            )));
        }
    });

    Effect::new({
        let scene = scene.clone();

        move || {
            let marks = marks.get();
            if editors_made.get() > 0 {
                state.editors.told(&scene, |editor| {
                    editor.set_quotation_marks(marks.opening(), marks.closing())
                });
            }
        }
    });

    Effect::new({
        let scene = scene.clone();

        move || {
            let labels = serde_json::json!({
                "missing": speech::say("figure-missing"),
                "caption": speech::say("figure-caption"),
                "dismiss": speech::say("problem-dismiss"),
            })
            .to_string();
            if editors_made.get() > 0 {
                state
                    .editors
                    .told(&scene, |editor| editor.set_labels(&labels));
            }
        }
    });

    on_cleanup({
        let scene = scene.clone();

        move || {
            if let Some(editor) = state.editors.gone(&scene) {
                editor.destroy();
            }
            state.saving.let_go_of(&scene);
            state.word_count.let_go_of(&scene);
            state.labels.let_go_of(&scene);
            state.presence.let_go_of(&scene);
            state.formatting.let_go_of(&scene);
            kept.set_value(None);
        }
    });

    html::div()
        .class("scene")
        .attr("data-scene", named)
        .node_ref(host)
}
