use leptos::html;
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::icons::{Icon, mark};
use crate::writing::write_view_state::WriteViewState;

pub const SCENE_TRAY: &str = "scene-tray";

pub fn this_scene(state: WriteViewState) -> impl IntoView {
    let tray = state.scene_tray;
    let label = Memo::new(move |_| tray.scene().map(|scene| state.labels.of_scene(&scene)));

    (
        html::button()
            .r#type("button")
            .class("scene-tray-tab")
            .attr("aria-expanded", move || tray.is_open().to_string())
            .attr("aria-controls", SCENE_TRAY)
            .on(ev::click, move |_| tray.toggle())
            .child((html::span().child("This scene"), move || {
                match tray.is_open() {
                    true => mark(Icon::PushIn).into_any(),
                    false => mark(Icon::PullOut).into_any(),
                }
            })),
        html::aside()
            .id(SCENE_TRAY)
            .class(SCENE_TRAY)
            .class(("open", move || tray.is_open()))
            .attr("aria-label", "This scene")
            .attr("hidden", move || (!tray.is_open()).then_some(""))
            .child(
                html::div().class("inside").child((
                    html::div().class("head").child((
                        html::p().class("tally").child("This scene"),
                        html::button()
                            .r#type("button")
                            .class("shut")
                            .attr("aria-label", "Close the tray")
                            .on(ev::click, move |_| tray.close())
                            .child("\u{00d7}"),
                    )),
                    move || match label.get() {
                        None => html::p()
                            .class("nothing-yet")
                            .child("Put the cursor in a scene to see it here.")
                            .into_any(),
                        Some(label) if label.shown.is_empty() => html::h2()
                            .class("scene-name untitled")
                            .child("An empty scene")
                            .into_any(),
                        Some(label) => html::h2()
                            .class("scene-name")
                            .class(("untitled", !label.titled))
                            .child(label.shown)
                            .into_any(),
                    },
                )),
            ),
    )
}
