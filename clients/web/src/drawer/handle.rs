use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use web_sys::Element;

use crate::drawer::width::DrawerWidth;

const STEP: f64 = 16.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Grows {
    Leftwards,
    Rightwards,
}

pub fn drawer_handle(
    width: DrawerWidth,
    grows: Grows,
    named: impl Fn() -> String + Send + Sync + 'static,
) -> impl IntoView {
    let dragging = StoredValue::new(None::<(f64, f64)>);
    let towards = move |moved: f64| match grows {
        Grows::Rightwards => moved,
        Grows::Leftwards => -moved,
    };

    html::div()
        .class("drawer-handle")
        .class(("leftwards", grows == Grows::Leftwards))
        .role("separator")
        .attr("tabindex", "0")
        .attr("aria-label", named)
        .attr("aria-orientation", "vertical")
        .attr("aria-valuenow", move || width.shown().round().to_string())
        .attr("aria-valuemin", move || {
            width.narrowest().round().to_string()
        })
        .attr("aria-valuemax", move || width.widest().round().to_string())
        .on(ev::pointerdown, move |event| {
            event.prevent_default();
            if let Some(handle) = event
                .current_target()
                .and_then(|target| target.dyn_into::<Element>().ok())
            {
                _ = handle.set_pointer_capture(event.pointer_id());
            }
            dragging.set_value(Some((f64::from(event.client_x()), width.shown_untracked())));
        })
        .on(ev::pointermove, move |event| {
            if let Some((from, started)) = dragging.get_value() {
                width.widen_to(started + towards(f64::from(event.client_x()) - from));
            }
        })
        .on(ev::pointerup, move |_| {
            if dragging.get_value().is_some() {
                dragging.set_value(None);
                width.keep();
            }
        })
        .on(ev::pointercancel, move |_| dragging.set_value(None))
        .on(ev::keydown, move |event| {
            let moved = match event.key().as_str() {
                "ArrowRight" => STEP,
                "ArrowLeft" => -STEP,
                _ => return,
            };
            event.prevent_default();
            width.widen_to(width.shown_untracked() + towards(moved));
            width.keep();
        })
}
