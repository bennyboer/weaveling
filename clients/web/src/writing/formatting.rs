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
use crate::writing::images;
use crate::writing::write_view_state::WriteViewState;

/// Where a selection is and how it is formatted, as the editor tells it.
/// The places are in pixels, measured from the scene's row: the top of the selection, its bottom, and its middle across.
/// `link` is the address the whole selection links to, if it is all one link.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Selected {
    above: f64,
    below: f64,
    across: f64,
    bold: bool,
    italic: bool,
    link: Option<String>,
    #[serde(default)]
    figure: Option<String>,
    #[serde(default)]
    empty_line: bool,
}

/// Which scene has text selected, and how, so its row can show the formatting toolbar.
/// While a link is being set, the toolbar shows a field for its address instead of its buttons.
#[derive(Clone, Copy)]
pub struct Formatting {
    shown: RwSignal<Option<(SceneId, Selected)>>,
    linking: RwSignal<bool>,
    refused: RwSignal<bool>,
}

impl Formatting {
    pub fn nothing() -> Self {
        Self {
            shown: RwSignal::new(None),
            linking: RwSignal::new(false),
            refused: RwSignal::new(false),
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
            self.linking.set(false);
            self.refused.set(false);
        }
    }

    /// Shows the field for a link's address, as Ctrl+K asks in the text.
    pub fn link_asked(self, scene: &SceneId) {
        if self.in_scene_untracked(scene) {
            self.refused.set(false);
            self.linking.set(true);
        }
    }

    fn in_scene_untracked(self, scene: &SceneId) -> bool {
        self.shown
            .with_untracked(|shown| shown.as_ref().is_some_and(|(at, _)| at == scene))
    }

    fn in_scene(self, scene: &SceneId) -> Option<Selected> {
        self.shown.with(|shown| {
            shown
                .as_ref()
                .filter(|(at, _)| at == scene)
                .map(|(_, selected)| selected.clone())
        })
    }

    fn done_linking(self) {
        self.linking.set(false);
        self.refused.set(false);
    }
}

/// Moves the keyboard into the toolbar of the scene, on its first button.
pub fn reached(scene: &SceneId) {
    if let Some(first) = document()
        .query_selector(&format!(
            "[data-scene-row=\"{scene}\"] :is(.formatting, .adding) button"
        ))
        .ok()
        .flatten()
        .and_then(|found| found.dyn_into::<HtmlElement>().ok())
    {
        let _ = first.focus();
    }
}

/// The toolbar shown over text selected in a scene: bold, italic, quotation marks, a link, and removing formatting.
/// It stands above the selection, or below it on a touch screen, where the browser shows its own menu above.
/// Pressing a button keeps the text focused. From the keyboard it is reached with Alt+F10,
/// the arrow keys move between its buttons, and Escape goes back to the text.
pub fn toolbar(scene: SceneId, state: WriteViewState) -> impl IntoView {
    move || {
        let selected = state.formatting.in_scene(&scene)?;
        let linking = state.formatting.linking.get();
        let placed = format!(
            "--above: {}px; --below: {}px; --across: {}px",
            selected.above, selected.below, selected.across
        );
        if let Some(size) = selected.figure.clone() {
            return Some(figure_buttons(scene.clone(), size, placed, state).into_any());
        }
        if selected.empty_line {
            return Some(adding(scene.clone(), placed, state).into_any());
        }

        Some(
            html::div()
                .class("formatting")
                .role("toolbar")
                .attr("aria-label", move || speech::say("formatting"))
                .style(format!(
                    "--above: {}px; --below: {}px; --across: {}px",
                    selected.above, selected.below, selected.across
                ))
                .child(if linking {
                    link_field(scene.clone(), selected.link.clone(), state).into_any()
                } else {
                    buttons(scene.clone(), &selected, state).into_any()
                })
                .into_any(),
        )
    }
}

/// The toolbar shown over a chosen image: its size, small, medium or the full width, and removing it.
fn figure_buttons(
    scene: SceneId,
    size: String,
    placed: String,
    state: WriteViewState,
) -> impl IntoView + use<> {
    let removing = scene.clone();
    let sized = move |named: &'static str, label: &'static str| {
        let scene = scene.clone();

        html::button()
            .r#type("button")
            .class("sized")
            .attr("aria-pressed", (size == named).to_string())
            .on(ev::pointerdown, |event| event.prevent_default())
            .on(ev::click, move |_| {
                _ = state
                    .editors
                    .told(&scene, |editor| editor.set_figure_size(named))
            })
            .child(speech::said(label))
    };

    html::div()
        .class("formatting figure-sizes")
        .role("toolbar")
        .attr("aria-label", move || speech::say("figure-toolbar"))
        .style(placed)
        .child((
            sized("small", "figure-small"),
            sized("medium", "figure-medium"),
            sized("full", "figure-full"),
            html::button()
                .r#type("button")
                .attr("aria-label", move || speech::say("figure-remove"))
                .attr("title", move || speech::say("figure-remove"))
                .on(ev::pointerdown, |event| event.prevent_default())
                .on(ev::click, move |_| {
                    _ = state.editors.told(&removing, ProseEditor::remove_figure)
                })
                .child(icons::mark(Icon::Trash)),
        ))
}

/// The "+" at the start of an empty line, offering what can be put there: for now an image.
/// It is left out of the Tab order, so Tab still leaves the scene; Alt+F10 reaches it, as it does the toolbar.
/// Choosing an image opens the scene's field for files, and the images go where the line is.
fn adding(scene: SceneId, placed: String, state: WriteViewState) -> impl IntoView + use<> {
    let open = RwSignal::new(false);
    let back = scene.clone();

    html::div()
        .class("adding")
        .style(placed)
        .on(ev::keydown, move |event| {
            if event.key() == "Escape" {
                event.prevent_default();
                event.stop_propagation();
                if open.get_untracked() {
                    open.set(false);
                } else {
                    state.editors.told(&back, ProseEditor::focus);
                }
            }
        })
        .child((
            html::button()
                .r#type("button")
                .class("add")
                .attr("aria-label", move || speech::say("adding-insert"))
                .attr("title", move || speech::say("adding-insert"))
                .attr("aria-haspopup", "menu")
                .attr("tabindex", "-1")
                .attr("aria-expanded", move || open.get().to_string())
                .on(ev::pointerdown, |event| event.prevent_default())
                .on(ev::click, move |_| open.update(|open| *open = !*open))
                .child(icons::mark(Icon::Plus)),
            move || {
                let scene = scene.clone();
                open.get().then(|| {
                    html::div().class("menu").role("menu").child(
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .on(ev::pointerdown, |event| event.prevent_default())
                            .on(ev::click, move |_| {
                                open.set(false);
                                if let Some(ticket) =
                                    state.editors.told(&scene, ProseEditor::asked_for_image)
                                {
                                    images::pick(&scene, ticket);
                                }
                            })
                            .child(speech::said("adding-image")),
                    )
                })
            },
        ))
}

fn buttons(scene: SceneId, selected: &Selected, state: WriteViewState) -> impl IntoView + use<> {
    let back = scene.clone();
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

    html::div()
        .class("formatting-buttons")
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
            html::button()
                .r#type("button")
                .attr("aria-label", move || speech::say("formatting-link"))
                .attr("title", move || speech::say("formatting-link"))
                .attr("aria-pressed", selected.link.is_some().to_string())
                .on(ev::pointerdown, |event| event.prevent_default())
                .on(ev::click, move |_| {
                    state.formatting.refused.set(false);
                    state.formatting.linking.set(true);
                })
                .child(icons::mark(Icon::Link)),
            button(
                Icon::ClearFormatting,
                "formatting-clear",
                None,
                ProseEditor::clear_formatting,
            ),
        ))
}

/// The field for a link's address, shown in the toolbar in place of its buttons.
/// An address without a scheme is taken as a web address, or as an email address if it looks like one.
/// Only web and email addresses are taken; anything else is refused and says so.
fn link_field(
    scene: SceneId,
    held: Option<String>,
    state: WriteViewState,
) -> impl IntoView + use<> {
    let address = RwSignal::new(held.clone().unwrap_or_default());
    let field = NodeRef::<html::Input>::new();
    field.on_load(|input| _ = input.focus());
    let back = StoredValue::new(scene.clone());
    let going_back = move || {
        state.formatting.done_linking();
        back.with_value(|back| state.editors.told(back, ProseEditor::focus));
    };
    let removing = scene.clone();

    html::form()
        .class("linking")
        .on(ev::submit, move |event| {
            event.prevent_default();
            let typed = address.get_untracked();
            if state.editors.told(&scene, |editor| editor.set_link(&typed)) == Some(true) {
                going_back();
            } else {
                state.formatting.refused.set(true);
            }
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Escape" {
                event.prevent_default();
                event.stop_propagation();
                going_back();
            }
        })
        .child((
            html::input()
                .node_ref(field)
                .r#type("text")
                .attr("inputmode", "url")
                .attr("aria-label", move || speech::say("formatting-link-address"))
                .attr("placeholder", move || {
                    speech::say("formatting-link-address")
                })
                .prop("value", move || address.get())
                .on(ev::input, move |event| {
                    address.set(event_target_value(&event))
                }),
            html::button()
                .r#type("submit")
                .child(speech::said("formatting-link-set")),
            held.is_some().then(|| {
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| {
                        state.editors.told(&removing, ProseEditor::remove_link);
                        going_back();
                    })
                    .child(speech::said("formatting-link-remove"))
            }),
            move || {
                state.formatting.refused.get().then(|| {
                    html::p()
                        .class("refused")
                        .role("alert")
                        .child(speech::said("formatting-link-refused"))
                })
            },
        ))
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
