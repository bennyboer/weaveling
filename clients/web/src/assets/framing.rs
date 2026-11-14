use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, ImageBitmap};

use crate::assets::squaring;
use crate::assets::squaring::{Framing, MOST_ZOOM, Square};
use crate::dialog;
use crate::speech;

/// How far one press of an arrow key moves the picture, as a share of the view's width.
const STEP: f64 = 0.05;

/// How much one turn of the mouse wheel zooms in or out.
const WHEEL_ZOOM: f64 = 1.1;

/// A dialog that lets the account choose which square of an image becomes its picture.
/// The picture is moved by dragging it or with the arrow keys, and zoomed with the slider or the mouse wheel.
/// A round outline shows what is seen where the picture is shown round.
pub fn framing_dialog(
    image: ImageBitmap,
    chosen: impl Fn(Square) + 'static,
    close: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let framing = RwSignal::new(Framing::of(image.width(), image.height()));
    let image = StoredValue::new_local(image);
    let canvas = NodeRef::<html::Canvas>::new();
    let pressed = StoredValue::new(None::<(i32, i32)>);

    Effect::new(move |_| {
        let square = framing.get().square();
        let Some(canvas) = canvas.get() else {
            return;
        };
        let ratio = window().device_pixel_ratio();
        let side = (f64::from(canvas.client_width()) * ratio).round().max(1.0) as u32;
        if canvas.width() != side {
            canvas.set_width(side);
            canvas.set_height(side);
        }
        image.with_value(|image| _ = squaring::drawn(image, square, &canvas));
    });
    let view = move || {
        canvas
            .get_untracked()
            .map_or(1.0, |canvas| f64::from(canvas.client_width()).max(1.0))
    };

    dialog::modal(
        Signal::derive(|| speech::say("picture-frame-heading")),
        "framing-picture",
        close,
        (
            html::p().child(speech::said("picture-frame-hint")),
            html::div()
                .class("framing")
                .attr("tabindex", "0")
                .role("group")
                .attr("aria-label", move || speech::say("picture-frame-move"))
                .on(ev::pointerdown, move |event| {
                    if let Some(under) = event
                        .current_target()
                        .and_then(|target| target.dyn_into::<HtmlElement>().ok())
                    {
                        let _ = under.set_pointer_capture(event.pointer_id());
                    }
                    pressed.set_value(Some((event.client_x(), event.client_y())));
                })
                .on(ev::pointermove, move |event| {
                    let Some((x, y)) = pressed.get_value() else {
                        return;
                    };
                    let (now_x, now_y) = (event.client_x(), event.client_y());
                    pressed.set_value(Some((now_x, now_y)));
                    framing.update(|framing| {
                        *framing =
                            framing.dragged(f64::from(now_x - x), f64::from(now_y - y), view())
                    });
                })
                .on(ev::pointerup, move |_| pressed.set_value(None))
                .on(ev::pointercancel, move |_| pressed.set_value(None))
                .on(ev::wheel, move |event| {
                    event.prevent_default();
                    let by = if event.delta_y() < 0.0 {
                        WHEEL_ZOOM
                    } else {
                        1.0 / WHEEL_ZOOM
                    };
                    framing.update(|framing| *framing = framing.zoomed(framing.zoom() * by));
                })
                .on(ev::keydown, move |event| {
                    let step = view() * STEP;
                    let (by_x, by_y) = match event.key().as_str() {
                        "ArrowLeft" => (-step, 0.0),
                        "ArrowRight" => (step, 0.0),
                        "ArrowUp" => (0.0, -step),
                        "ArrowDown" => (0.0, step),
                        _ => return,
                    };
                    event.prevent_default();
                    framing.update(|framing| *framing = framing.dragged(by_x, by_y, view()));
                })
                .child((
                    html::canvas().node_ref(canvas).attr("aria-hidden", "true"),
                    html::div().class("ring").attr("aria-hidden", "true"),
                )),
            html::label().class("field zooming").child((
                html::span().child(speech::said("picture-frame-zoom")),
                html::input()
                    .r#type("range")
                    .attr("min", "1")
                    .attr("max", MOST_ZOOM.to_string())
                    .attr("step", "0.01")
                    .prop("value", move || framing.get().zoom().to_string())
                    .on(ev::input, move |event| {
                        if let Ok(zoom) = event_target_value(&event).parse::<f64>() {
                            framing.update(|framing| *framing = framing.zoomed(zoom));
                        }
                    }),
            )),
            html::div().class("dialog-actions").child((
                html::button()
                    .r#type("button")
                    .on(ev::click, move |_| close())
                    .child(speech::said("picture-cancel")),
                html::button()
                    .r#type("button")
                    .class("primary")
                    .on(ev::click, move |_| chosen(framing.get_untracked().square()))
                    .child(speech::said("picture-frame-confirm")),
            )),
        ),
    )
}
