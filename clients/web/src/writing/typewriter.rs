use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event, HtmlElement, Node};

use crate::writing::write_view_state::WriteViewState;

pub fn typewriter_scrolling(state: WriteViewState) {
    let pressed = StoredValue::new(false);

    let pressing = window_event_listener(ev::pointerdown, move |_| pressed.set_value(true));
    let letting_go = window_event_listener(ev::pointerup, move |_| {
        pressed.set_value(false);
        if state.settings.scrolls_like_a_typewriter() {
            to_the_middle();
        }
    });
    let moved = Closure::<dyn Fn(Event)>::new(move |_: Event| {
        if !pressed.get_value() && state.settings.scrolls_like_a_typewriter() {
            to_the_middle();
        }
    });
    _ = document()
        .add_event_listener_with_callback("selectionchange", moved.as_ref().unchecked_ref());
    let moved = StoredValue::new_local(moved);

    Effect::new(move || {
        let on = state.settings.scrolls_like_a_typewriter();
        if !state.structure.is_open() {
            return;
        }
        _ = request_animation_frame(move || match on {
            true => make_room_above(),
            false => let_room_above_go(),
        });
    });
    let resized = window_event_listener(ev::resize, move |_| {
        if state.settings.scrolls_like_a_typewriter() {
            make_room_above();
        }
    });

    on_cleanup(move || {
        pressing.remove();
        letting_go.remove();
        resized.remove();
        moved.with_value(|moved| {
            _ = document().remove_event_listener_with_callback(
                "selectionchange",
                moved.as_ref().unchecked_ref(),
            );
        });
    });
}

fn to_the_middle() {
    let Some(line) = caret_line() else {
        return;
    };
    let Some(height) = window()
        .inner_height()
        .ok()
        .and_then(|height| height.as_f64())
    else {
        return;
    };
    let off_by = line - height / 2.0;

    if off_by.abs() > 2.0 {
        window().scroll_by_with_x_and_y(0.0, off_by);
    }
}

fn caret_line() -> Option<f64> {
    let selection = window().get_selection().ok()??;
    let focus: Node = selection.focus_node()?;
    let writing = match focus.dyn_ref::<Element>() {
        Some(element) => element.clone(),
        None => focus.parent_element()?,
    };
    writing
        .closest("article.writing [contenteditable]")
        .ok()??;

    let range = selection.get_range_at(0).ok()?;
    let rect = range.get_bounding_client_rect();
    if rect.height() > 0.0 {
        return Some(rect.top() + rect.height() / 2.0);
    }
    let around = writing.get_bounding_client_rect();

    Some(around.top() + around.height().min(40.0) / 2.0)
}

const LEAST_ROOM_ABOVE: f64 = 56.0;

fn book() -> Option<HtmlElement> {
    document()
        .query_selector("article.writing")
        .ok()??
        .dyn_into::<HtmlElement>()
        .ok()
}

fn make_room_above() {
    let Some(book) = book() else {
        return;
    };
    let Some(first) = book.query_selector(".heading, .ProseMirror").ok().flatten() else {
        return;
    };
    let (Some(height), Ok(scrolled)) = (
        window()
            .inner_height()
            .ok()
            .and_then(|height| height.as_f64()),
        window().scroll_y(),
    ) else {
        return;
    };
    let room = window()
        .get_computed_style(&book)
        .ok()
        .flatten()
        .and_then(|style| style.get_property_value("padding-top").ok())
        .and_then(|room| room.trim_end_matches("px").parse::<f64>().ok())
        .unwrap_or(0.0);
    let line = first.get_bounding_client_rect();
    let in_the_page = line.top() + scrolled + line.height().min(64.0) / 2.0;
    let wanted = (room + height / 2.0 - in_the_page).max(LEAST_ROOM_ABOVE);

    _ = book
        .style()
        .set_property("padding-top", &format!("{wanted}px"));
}

fn let_room_above_go() {
    if let Some(book) = book() {
        _ = book.style().remove_property("padding-top");
    }
}
