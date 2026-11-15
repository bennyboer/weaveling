use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use serde::Deserialize;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::icons;
use crate::icons::Icon;
use crate::scenes::model::SceneId;
use crate::scenes::prose::ProseEditor;
use crate::speech;
use crate::writing::write_view_state::WriteViewState;

/// Where a selection is and how it is formatted, as the editor tells it.
/// The places are in pixels, measured from the scene's row: the top of the selection, its bottom, and its middle across.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Selected {
    above: f64,
    below: f64,
    across: f64,
    bold: bool,
    italic: bool,
}

/// Which scene has text selected, and how, so its row can show the formatting toolbar.
#[derive(Clone, Copy)]
pub struct Formatting {
    shown: RwSignal<Option<(SceneId, Selected)>>,
}

impl Formatting {
    pub fn nothing() -> Self {
        Self {
            shown: RwSignal::new(None),
        }
    }

    /// Takes in what a scene's editor tells about its selection.
    /// Nothing told, or nothing readable, means no text is selected any more.
    pub fn told(self, scene: &SceneId, told: &str) {
        match serde_json::from_str::<Selected>(told) {
            Ok(selected) => self.shown.set(Some((scene.clone(), selected))),
            Err(_) => self.let_go_of(scene),
        }
    }

    /// Hides the toolbar if it is shown for the given scene.
    pub fn let_go_of(self, scene: &SceneId) {
        if self
            .shown
            .with_untracked(|shown| shown.as_ref().is_some_and(|(at, _)| at == scene))
        {
            self.shown.set(None);
        }
    }

    fn in_scene(self, scene: &SceneId) -> Option<Selected> {
        self.shown.with(|shown| {
            shown
                .as_ref()
                .filter(|(at, _)| at == scene)
                .map(|(_, selected)| *selected)
        })
    }
}

/// Moves the keyboard into the toolbar of the scene, on its first button.
pub fn reached(scene: &SceneId) {
    if let Some(first) = document()
        .query_selector(&format!("[data-scene-row=\"{scene}\"] .formatting button"))
        .ok()
        .flatten()
        .and_then(|found| found.dyn_into::<HtmlElement>().ok())
    {
        let _ = first.focus();
    }
}

/// The toolbar shown over text selected in a scene: bold, italic, quotation marks, and removing formatting.
/// It stands above the selection, or below it on a touch screen, where the browser shows its own menu above.
/// Pressing a button keeps the text focused. From the keyboard it is reached with Alt+F10,
/// the arrow keys move between its buttons, and Escape goes back to the text.
pub fn toolbar(scene: SceneId, state: WriteViewState) -> impl IntoView {
    move || {
        let selected = state.formatting.in_scene(&scene)?;
        let back = scene.clone();
        let scene = scene.clone();
        let button =
            move |icon: Icon, label: &'static str, pressed: Option<bool>, act: fn(&ProseEditor)| {
                let scene = scene.clone();

                html::button()
                    .r#type("button")
                    .attr("aria-label", move || speech::say(label))
                    .attr("title", move || speech::say(label))
                    .attr("aria-pressed", pressed.map(|pressed| pressed.to_string()))
                    .on(ev::pointerdown, |event| event.prevent_default())
                    .on(ev::click, move |_| _ = state.editors.told(&scene, act))
                    .child(icons::mark(icon))
            };

        Some(
            html::div()
                .class("formatting")
                .role("toolbar")
                .attr("aria-label", move || speech::say("formatting"))
                .style(format!(
                    "--above: {}px; --below: {}px; --across: {}px",
                    selected.above, selected.below, selected.across
                ))
                .on(ev::keydown, move |event| {
                    let key = event.key();
                    if key == "Escape" {
                        event.prevent_default();
                        event.stop_propagation();
                        state.editors.told(&back, ProseEditor::focus);
                        return;
                    }
                    let step: i32 = match key.as_str() {
                        "ArrowRight" => 1,
                        "ArrowLeft" => -1,
                        _ => return,
                    };
                    event.prevent_default();
                    event.stop_propagation();
                    moved_within(&event, step);
                })
                .child((
                    button(
                        Icon::Bold,
                        "formatting-bold",
                        Some(selected.bold),
                        ProseEditor::toggle_bold,
                    ),
                    button(
                        Icon::Italic,
                        "formatting-italic",
                        Some(selected.italic),
                        ProseEditor::toggle_italic,
                    ),
                    button(
                        Icon::Quote,
                        "formatting-quote",
                        None,
                        ProseEditor::quote_selection,
                    ),
                    button(
                        Icon::ClearFormatting,
                        "formatting-clear",
                        None,
                        ProseEditor::clear_formatting,
                    ),
                )),
        )
    }
}

/// Moves the focus to the next or the previous button of the toolbar, going round at its ends.
fn moved_within(event: &ev::KeyboardEvent, step: i32) {
    let Some(bar) = event
        .current_target()
        .and_then(|target| target.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    let Ok(buttons) = bar.query_selector_all("button") else {
        return;
    };
    let count = buttons.length() as i32;
    if count == 0 {
        return;
    }
    let focused = document().active_element();
    let at = (0..count)
        .find(|nth| {
            buttons
                .item(*nth as u32)
                .is_some_and(|button| Some(button) == focused.clone().map(Into::into))
        })
        .unwrap_or(0);
    let next = (at + step).rem_euclid(count);
    if let Some(button) = buttons
        .item(next as u32)
        .and_then(|button| button.dyn_into::<HtmlElement>().ok())
    {
        let _ = button.focus();
    }
}
