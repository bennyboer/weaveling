use leptos::html::{Div, Span};
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use web_sys::{Element, File, FileList, HtmlElement};

use crate::icons;
use crate::icons::Icon;
use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::speech;
use crate::writing::beginnings::{CarriedImage, Pending};
use crate::writing::heading;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

const INVITATION: &str = "scene-invitation";

const NEXT: &str = "scene-next";

#[derive(Clone)]
enum Line {
    Opening,
    After(SceneId),
    First,
}

pub fn opening(section: SectionId, state: WriteViewState) -> impl IntoView {
    scene_line(section, Line::Opening, state)
}

pub fn pending_scene(section: SectionId, after: SceneId, state: WriteViewState) -> impl IntoView {
    scene_line(section, Line::After(after), state)
}

pub fn pending_first_scene(section: SectionId, state: WriteViewState) -> impl IntoView {
    scene_line(section, Line::First, state)
}

/// The line a new scene begins on, until something is typed or given there.
/// Typing begins the scene and carries the words into it. Image files pasted or dropped there begin it too,
/// and are uploaded into it. While the author is at the empty line, having clicked or pressed a key there,
/// and not merely had it focused by the view, a "+" at the end of its placeholder
/// offers an image, chosen in the dialog for images, to begin the scene with.
fn scene_line(section: SectionId, line: Line, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let around: NodeRef<Div> = NodeRef::new();
    let measured: NodeRef<Span> = NodeRef::new();
    let attended = RwSignal::new(false);
    let empty = RwSignal::new(true);
    let stop = Stop::Opening(section.clone());
    let after = match &line {
        Line::After(scene) => Some(scene.clone()),
        Line::Opening | Line::First => None,
    };
    let pending = match &line {
        Line::Opening => None,
        Line::After(_) | Line::First => Some(Pending::Scene {
            section: section.clone(),
            after: after.clone(),
        }),
    };

    match &pending {
        Some(_) => focus_when_mounted(host),
        None => {
            Effect::new({
                let stop = stop.clone();

                move || {
                    if let Some(shown) = host.get() {
                        state.walk.line_at(stop.clone(), (*shown).clone());
                    }
                }
            });
            on_cleanup({
                let stop = stop.clone();

                move || state.walk.line_gone(&stop)
            });
        }
    }

    let begun = StoredValue::new((section.clone(), after.clone()));
    let given = move |files: Option<FileList>| {
        let images: Vec<File> = files
            .map(|files| {
                (0..files.length())
                    .filter_map(|nth| files.get(nth))
                    .filter(|file| file.type_().starts_with("image/"))
                    .collect()
            })
            .unwrap_or_default();
        if images.is_empty() {
            return false;
        }
        let (section, after) = begun.get_value();
        if let Some(shown) = host.get_untracked() {
            state
                .beginnings
                .opening_words_are(section.clone(), shown.text_content().unwrap_or_default());
        }
        state
            .beginnings
            .opening_image_given(section.clone(), CarriedImage::Files(images));
        state.beginnings.begin_scene(section, after);

        true
    };
    let within = move |target: Option<web_sys::EventTarget>| {
        let Some(around) = around.get_untracked() else {
            return false;
        };
        target
            .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
            .is_some_and(|target| around.contains(Some(&target)))
    };

    html::div().class("opening-line").node_ref(around).child((
        html::div()
            .class("opening")
            .attr("contenteditable", "plaintext-only")
            .attr("role", "textbox")
            .attr("aria-label", speech::said(NEXT))
            .attr("data-placeholder", speech::said(INVITATION))
            .node_ref(host)
            .on(ev::input, {
                let section = section.clone();
                let after = after.clone();

                move |_| {
                    if let Some(shown) = host.get_untracked() {
                        let typed = shown.text_content().unwrap_or_default();
                        empty.set(typed.is_empty());
                        state.beginnings.opening_words_are(section.clone(), typed);
                    }
                    state.beginnings.begin_scene(section.clone(), after.clone());
                }
            })
            .on(ev::paste, move |event| {
                if given(event.clipboard_data().and_then(|data| data.files())) {
                    event.prevent_default();
                }
            })
            .on(ev::drop, move |event| {
                if given(event.data_transfer().and_then(|data| data.files())) {
                    event.prevent_default();
                }
            })
            .on(ev::focus, move |_| caret_at_the_start(host))
            .on(ev::pointerup, move |_| {
                caret_at_the_start(host);
                attended.set(true);
            })
            .on(ev::keyup, move |_| attended.set(true))
            .on(ev::blur, {
                let pending = pending.clone();

                move |event| {
                    if within(event.related_target()) {
                        return;
                    }
                    attended.set(false);
                    let empty = host
                        .get_untracked()
                        .is_none_or(|shown| shown.text_content().unwrap_or_default().is_empty());

                    if let (true, Some(pending)) = (empty, &pending) {
                        state.beginnings.drop_pending(pending);
                    }
                }
            })
            .on(ev::keydown, move |event| {
                if event.key() == "F10" && event.alt_key() {
                    event.prevent_default();
                    attended.set(true);
                    _ = request_animation_frame(move || {
                        if let Some(add) = around
                            .get_untracked()
                            .and_then(|around| {
                                around.query_selector(".adding button").ok().flatten()
                            })
                            .and_then(|add| add.dyn_into::<HtmlElement>().ok())
                        {
                            let _ = add.focus();
                        }
                    });

                    return;
                }
                if event.key() == "Enter" {
                    event.prevent_default();
                    state.beginnings.start_pending_section_after(&section);

                    return;
                }

                let anchor = match &line {
                    Line::Opening => return heading::walk_on(&event, host, stop.clone(), state),
                    Line::After(scene) => Stop::Scene(scene.clone()),
                    Line::First => Stop::Heading(section.clone()),
                };
                let moved = match event.key().as_str() {
                    "ArrowUp" => state.walk.arrive(&anchor, Towards::Before),
                    "ArrowDown" => state.walk.leave(&anchor, Towards::After),
                    _ => false,
                };

                if moved {
                    event.prevent_default();
                }
            }),
        html::span()
            .class("opening-measure")
            .attr("aria-hidden", "true")
            .node_ref(measured)
            .child(speech::said(INVITATION)),
        move || {
            (attended.get() && empty.get()).then(|| {
                adding(
                    begun,
                    placed(
                        host.get_untracked().map(|line| (*line).clone()),
                        measured.get_untracked().map(|span| (*span).clone()),
                    ),
                    host,
                    attended,
                    state,
                )
            })
        },
    ))
}

/// Puts the caret at the start of an empty line.
/// The line holds an invisible marker, and a click or focus may put the caret after it,
/// which shows it at the far end of the line, past the placeholder.
fn caret_at_the_start(host: NodeRef<Div>) {
    let Some(line) = host.get_untracked() else {
        return;
    };
    if line.text_content().unwrap_or_default().is_empty()
        && let Ok(Some(selection)) = window().get_selection()
    {
        let _ = selection.collapse_with_offset(Some(&line), 0);
    }
}

/// Where the "+" stands: after the placeholder, centred on the line, or under it where the line has no room.
fn placed(line: Option<HtmlElement>, measured: Option<HtmlElement>) -> String {
    let (Some(line), Some(measured)) = (line, measured) else {
        return String::new();
    };
    let high = line.offset_height();
    let across = measured.offset_width();

    match across + 48 > line.offset_width() {
        true => format!(
            "--above: {high}px; --below: {}px; --across: -12px",
            high * 2
        ),
        false => format!("--above: 0px; --below: {high}px; --across: {across}px"),
    }
}

/// The "+" on the line a new scene begins on, offering an image to begin it with.
/// It is left out of the Tab order; Alt+F10 reaches it, and Escape goes back to the line.
fn adding(
    begun: StoredValue<(SectionId, Option<SceneId>)>,
    placed: String,
    host: NodeRef<Div>,
    attended: RwSignal<bool>,
    state: WriteViewState,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let back = move || {
        if let Some(line) = host.get_untracked() {
            let _ = line.focus();
        }
    };

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
                    back();
                }
            }
        })
        .on(ev::focusout, move |event| {
            let left = event
                .related_target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .is_none_or(|target| target.closest(".opening-line").ok().flatten().is_none());
            if left {
                attended.set(false);
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
                open.get().then(|| {
                    html::div().class("menu").role("menu").child(
                        html::button()
                            .r#type("button")
                            .role("menuitem")
                            .on(ev::pointerdown, |event| event.prevent_default())
                            .on(ev::click, move |_| {
                                open.set(false);
                                let (section, after) = begun.get_value();
                                state.image_choice.ask_for_beginning(section, after);
                            })
                            .child(speech::said("adding-image")),
                    )
                })
            },
        ))
}

fn focus_when_mounted(host: NodeRef<Div>) {
    Effect::new(move || {
        if let Some(shown) = host.get() {
            let _ = shown.focus();
        }
    });
}
