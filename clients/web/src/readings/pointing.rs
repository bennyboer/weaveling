use std::cell::RefCell;
use std::time::Duration;

use js_sys::{Array, Function, Reflect};
use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, HtmlElement, Node, Range, ScrollIntoViewOptions, ScrollLogicalPosition};

use crate::feedback::model::ReadingCopyPlace;
use crate::readings::view;

/// How long a place pointed at pulses, so the eye finds it: as long as its animation runs.
const POINTED_AT_FOR: Duration = Duration::from_millis(1800);
/// The mark of a place pointed at: a class on the element it lies in, and the name of the highlight that paints a passage.
const POINTED_AT: &str = "pointed-at";
/// The class that runs the pulse a passage's highlight takes its colour from.
const POINTING: &str = "pointing";

/// The place pulsing now, and the timer that will end its pulse.
struct Pulsing {
    found: Element,
    timer: Option<TimeoutHandle>,
}

thread_local! {
    static PULSING: RefCell<Option<Pulsing>> = const { RefCell::new(None) };
}

/// Ends the pulse of the place pointed at before, at once, so a new one starts afresh and no earlier timer cuts it short.
fn stop_pulsing() {
    if let Some(Pulsing { found, timer }) = PULSING.with_borrow_mut(Option::take) {
        if let Some(timer) = timer {
            timer.clear();
        }
        _ = found.class_list().remove_2(POINTING, POINTED_AT);
        unhighlighted(POINTED_AT);
    }
}

/// Brings a place in the copy into sight and makes it pulse a moment:
/// a passage's own words where the browser can paint them, else the block, a scene, a section's heading, or the work's title.
/// Pointing at another place, or the same again, ends the pulse before it.
pub(crate) fn point_at(place: &ReadingCopyPlace) {
    stop_pulsing();
    let Some(found) = element_of(place) else {
        return;
    };
    let in_the_middle = ScrollIntoViewOptions::new();
    in_the_middle.set_block(ScrollLogicalPosition::Center);
    found.scroll_into_view_with_scroll_into_view_options(&in_the_middle);

    let painted = match place {
        ReadingCopyPlace::Passage { from, to, .. } => stretch_in(&found, *from, *to)
            .is_some_and(|words| highlighted(POINTED_AT, std::slice::from_ref(&words))),
        _ => false,
    };
    pulsing(found, if painted { POINTING } else { POINTED_AT });
}

/// Brings an element into sight, as little scrolled as it can be, and makes it pulse a moment, ending the pulse before it.
pub(crate) fn pulse(found: Element) {
    stop_pulsing();
    let nearby = ScrollIntoViewOptions::new();
    nearby.set_block(ScrollLogicalPosition::Nearest);
    found.scroll_into_view_with_scroll_into_view_options(&nearby);
    pulsing(found, POINTED_AT);
}

fn pulsing(found: Element, class: &str) {
    if let Some(shown) = found.dyn_ref::<HtmlElement>() {
        _ = shown.offset_width();
    }
    _ = found.class_list().add_1(class);
    let timer = set_timeout(stop_pulsing, POINTED_AT_FOR).ok();
    PULSING.with_borrow_mut(|pulsing| *pulsing = Some(Pulsing { found, timer }));
}

fn element_of(place: &ReadingCopyPlace) -> Option<Element> {
    let page = document();
    match place {
        ReadingCopyPlace::Passage {
            scene, block_path, ..
        } => page
            .query_selector(&format!(
                ".reading-view .scene[data-scene=\"{scene}\"] [data-block=\"{}\"]",
                view::path_written(block_path)
            ))
            .ok()
            .flatten(),
        ReadingCopyPlace::Scene(scene) => page
            .query_selector(&format!(".reading-view .scene[data-scene=\"{scene}\"]"))
            .ok()
            .flatten(),
        ReadingCopyPlace::Section(section) => page.get_element_by_id(&view::heading_id(section)),
        ReadingCopyPlace::Whole => page.query_selector(".reading-view .book h1").ok().flatten(),
    }
}

/// A passage's own words in the text, if they are drawn.
pub(crate) fn words_of(place: &ReadingCopyPlace) -> Option<Range> {
    let ReadingCopyPlace::Passage { from, to, .. } = place else {
        return None;
    };

    stretch_in(&element_of(place)?, *from, *to)
}

/// The stretch of a block's text from one character to another, counted as a passage counts them.
pub(crate) fn stretch_in(block: &Element, from: u32, to: u32) -> Option<Range> {
    let mut texts = Vec::new();
    texts_in(block.as_ref(), &mut texts);
    let (start, start_at) = position_in(&texts, from)?;
    let (end, end_at) = position_in(&texts, to)?;
    let words = document().create_range().ok()?;
    words.set_start(&start, start_at).ok()?;
    words.set_end(&end, end_at).ok()?;

    Some(words)
}

fn texts_in(node: &Node, texts: &mut Vec<Node>) {
    if node.node_type() == Node::TEXT_NODE {
        texts.push(node.clone());
        return;
    }
    let children = node.child_nodes();
    for at in 0..children.length() {
        if let Some(child) = children.item(at) {
            texts_in(&child, texts);
        }
    }
}

/// Where the character so many in lies: its text node, and the offset in it as the browser counts, in UTF-16.
fn position_in(texts: &[Node], chars: u32) -> Option<(Node, u32)> {
    let mut left = usize::try_from(chars).ok()?;
    for text in texts {
        let said = text.text_content().unwrap_or_default();
        let held = said.chars().count();
        if left <= held {
            let offset: usize = said.chars().take(left).map(char::len_utf16).sum();
            return Some((text.clone(), u32::try_from(offset).ok()?));
        }
        left -= held;
    }

    None
}

/// The page's highlights, where the browser keeps them; none where it cannot paint a stretch of text.
fn highlights() -> Option<JsValue> {
    let css = Reflect::get(&window(), &JsValue::from_str("CSS")).ok()?;
    let held = Reflect::get(&css, &JsValue::from_str("highlights")).ok()?;

    (!held.is_undefined()).then_some(held)
}

/// Paints these stretches of words under the name given, as `::highlight(name)` styles them; whether the browser could.
pub(crate) fn highlighted(name: &str, words: &[Range]) -> bool {
    let painted = || -> Option<()> {
        let registry = highlights()?;
        let making = Reflect::get(&window(), &JsValue::from_str("Highlight"))
            .ok()?
            .dyn_into::<Function>()
            .ok()?;
        let stretches: Array = words.iter().collect();
        let highlight = Reflect::construct(&making, &stretches).ok()?;
        let set = Reflect::get(&registry, &JsValue::from_str("set"))
            .ok()?
            .dyn_into::<Function>()
            .ok()?;
        set.call2(&registry, &JsValue::from_str(name), &highlight)
            .ok()?;

        Some(())
    };

    painted().is_some()
}

pub(crate) fn unhighlighted(name: &str) {
    let Some(registry) = highlights() else {
        return;
    };
    if let Some(delete) = Reflect::get(&registry, &JsValue::from_str("delete"))
        .ok()
        .and_then(|delete| delete.dyn_into::<Function>().ok())
    {
        _ = delete.call1(&registry, &JsValue::from_str(name));
    }
}
