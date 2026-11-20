use std::time::Duration;

use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Node, Range};

use crate::feedback::column::Commenting;
use crate::feedback::model::ReadingCopyPlace;
use crate::icons;
use crate::icons::Icon;
use crate::scenes::model::SceneId;
use crate::speech;

/// A passage selected in the text, offered to be commented on, and where the offer stands below it in the text.
#[derive(Clone, Debug, PartialEq)]
struct Offer {
    place: ReadingCopyPlace,
    top: f64,
    left: f64,
}

/// How long a selection must rest before it is offered to be commented on, when no release of a button or a key ends it,
/// as when a selection is made and drawn out by touch.
const SETTLED_AFTER: Duration = Duration::from_millis(250);

/// The offer to comment on what is selected in a copy's text: a button by the selection,
/// shown once a selection is made and gone once it is.
/// A selection made with the mouse or the keys is offered as the button or key is let go;
/// one made by touch, which lets go of nothing the page hears, once it has rested a moment.
pub fn offered(commenting: Commenting) -> impl IntoView {
    let offer = RwSignal::new(None::<Offer>);
    let pressing = StoredValue::new(false);
    let resting = StoredValue::new_local(None::<TimeoutHandle>);
    let looked = move || _ = offer.try_set(selected());
    let pressed = window_event_listener(ev::pointerdown, move |event| {
        if event.pointer_type() == "mouse" {
            pressing.set_value(true);
        }
    });
    let after_pointer = window_event_listener(ev::pointerup, move |_| {
        pressing.set_value(false);
        looked();
    });
    let after_keys = window_event_listener(ev::keyup, move |_| looked());
    let changed: Closure<dyn Fn()> = Closure::new(move || {
        if let Some(waiting) = resting.try_update_value(Option::take).flatten() {
            waiting.clear();
        }
        let nothing_selected = window()
            .get_selection()
            .ok()
            .flatten()
            .is_none_or(|selection| selection.is_collapsed());
        if nothing_selected {
            _ = offer.try_set(None);
        } else if !pressing.get_value() {
            resting.set_value(set_timeout(looked, SETTLED_AFTER).ok());
        }
    });
    _ = document()
        .add_event_listener_with_callback("selectionchange", changed.as_ref().unchecked_ref());
    let changed = StoredValue::new_local(Some(changed));
    on_cleanup(move || {
        pressed.remove();
        after_pointer.remove();
        after_keys.remove();
        if let Some(waiting) = resting.try_update_value(Option::take).flatten() {
            waiting.clear();
        }
        if let Some(changed) = changed.try_update_value(Option::take).flatten() {
            _ = document().remove_event_listener_with_callback(
                "selectionchange",
                changed.as_ref().unchecked_ref(),
            );
        }
    });

    move || {
        offer.get().map(|Offer { place, top, left }| {
            html::button()
                .r#type("button")
                .class("comment-passage")
                .style(("top", format!("{top}px")))
                .style(("left", format!("{left}px")))
                .on(ev::pointerdown, |event| event.prevent_default())
                .on(ev::click, move |_| {
                    commenting.begin(place.clone());
                    if let Ok(Some(selection)) = window().get_selection() {
                        _ = selection.remove_all_ranges();
                    }
                    offer.set(None);
                })
                .child((
                    icons::mark(Icon::Comment),
                    html::span().child(speech::said("feedback-comment-on-passage")),
                ))
        })
    }
}

/// The passage selected in the text, if any, and where to offer commenting on it.
/// A passage lies within one block: a selection that runs on past its first block is cut at that block's end.
/// Its edges are trimmed of white space, and its stretch is counted in characters from the block's start.
fn selected() -> Option<Offer> {
    let selection = window().get_selection().ok()??;
    if selection.is_collapsed() || selection.range_count() == 0 {
        return None;
    }
    let range = selection.get_range_at(0).ok()?;
    let block = element_of(&range.start_container().ok()?)?
        .closest("[data-block]")
        .ok()??;
    let scene = block.closest("[data-scene]").ok()??;
    let book = block.closest(".book").ok()??;

    let cut = range.clone_range();
    let block_node: &Node = block.as_ref();
    if !block_node.contains(Some(&range.end_container().ok()?)) {
        cut.set_end(block_node, block_node.child_nodes().length())
            .ok()?;
    }
    let before = document().create_range().ok()?;
    before.set_start(block_node, 0).ok()?;
    before
        .set_end(&cut.start_container().ok()?, cut.start_offset().ok()?)
        .ok()?;

    let chosen = String::from(cut.to_string());
    let quote = chosen.trim();
    if quote.is_empty() {
        return None;
    }
    let leading = chosen.chars().count() - chosen.trim_start().chars().count();
    let from = String::from(before.to_string()).chars().count() + leading;
    let to = from + quote.chars().count();

    let shown = last_line_of(&cut)?;
    let within = book.get_bounding_client_rect();

    Some(Offer {
        place: ReadingCopyPlace::Passage {
            scene: SceneId::from(scene.get_attribute("data-scene")?),
            block_path: block
                .get_attribute("data-block")?
                .split('.')
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?,
            from: u32::try_from(from).ok()?,
            to: u32::try_from(to).ok()?,
            quote: quote.to_owned(),
        },
        top: shown.bottom() - within.top() + 6.0,
        left: (shown.right() - within.left()).max(0.0),
    })
}

fn element_of(node: &Node) -> Option<Element> {
    match node.dyn_ref::<Element>() {
        Some(element) => Some(element.clone()),
        None => node.parent_element(),
    }
}

/// The last line the selection covers, below which the offer stands.
fn last_line_of(range: &Range) -> Option<web_sys::DomRect> {
    let lines = range.get_client_rects()?;
    let count = lines.length();

    lines.get(count.checked_sub(1)?)
}
