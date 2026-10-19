use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};

struct Held {
    element: Element,
    anchor: Option<(Node, u32)>,
    focus: Option<(Node, u32)>,
}

pub fn keeping_the_cursor(change: impl FnOnce()) {
    let held = held_now();
    change();

    if let Some(held) = held {
        queue_microtask(move || put_back(held));
    }
}

fn held_now() -> Option<Held> {
    let element = document().active_element()?;
    if element == document().body()?.into() {
        return None;
    }
    let selection = window().get_selection().ok().flatten();
    let anchor = selection
        .as_ref()
        .and_then(|selection| Some((selection.anchor_node()?, selection.anchor_offset())));
    let focus = selection
        .as_ref()
        .and_then(|selection| Some((selection.focus_node()?, selection.focus_offset())));

    Some(Held {
        element,
        anchor,
        focus,
    })
}

fn put_back(held: Held) {
    let still_held = document()
        .active_element()
        .is_some_and(|now| now == held.element);
    if still_held || !held.element.is_connected() {
        return;
    }
    if let Some(focusable) = held.element.dyn_ref::<HtmlElement>() {
        _ = focusable.focus();
    }
    if let (Some((anchor, at)), Some((focus, to)), Ok(Some(selection))) =
        (held.anchor, held.focus, window().get_selection())
    {
        _ = selection.set_base_and_extent(&anchor, at, &focus, to);
    }
}
