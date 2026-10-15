use leptos::html::{self, Input};
use leptos::prelude::*;
use leptos::{IntoView, ev};

use crate::icons::{Icon, mark};
use crate::scenes::model::SceneId;
use crate::writing::write_view_state::WriteViewState;

pub const SCENE_TRAY: &str = "scene-tray";

pub fn this_scene(state: WriteViewState) -> impl IntoView {
    let tray = state.scene_tray;

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
                    move || match tray.scene() {
                        None => html::p()
                            .class("nothing-yet")
                            .child("Put the cursor in a scene to see it here.")
                            .into_any(),
                        Some(scene) => title_field(scene, state).into_any(),
                    },
                )),
            ),
    )
}

fn title_field(scene: SceneId, state: WriteViewState) -> impl IntoView {
    let field: NodeRef<Input> = NodeRef::new();
    let label = Memo::new({
        let scene = scene.clone();

        move |_| state.labels.of_scene(&scene)
    });
    let titled = move || label.with(|label| label.titled.then(|| label.shown.clone()));

    let rename = {
        let scene = scene.clone();

        move || {
            if let Some(typed) = field.get_untracked() {
                state
                    .labels
                    .rename(scene.clone(), typed.value().trim().to_owned());
            }
        }
    };

    html::input()
        .r#type("text")
        .class("scene-name")
        .attr("aria-label", "Scene title")
        .attr("placeholder", move || {
            label.with(|label| match label.shown.as_str() {
                "" => "An empty scene".to_owned(),
                opening if !label.titled => opening.to_owned(),
                _ => String::new(),
            })
        })
        .prop("value", move || titled().unwrap_or_default())
        .node_ref(field)
        .on(ev::change, {
            let rename = rename.clone();

            move |_| rename()
        })
        .on(ev::keydown, move |event| match event.key().as_str() {
            "Enter" => {
                event.prevent_default();
                rename();
                if !state.walk.back_into(&scene) {
                    state.left_off.land(Some(scene.clone()));
                }
            }
            "Escape" => {
                event.prevent_default();
                event.stop_propagation();
                if let Some(typed) = field.get_untracked() {
                    typed.set_value(&titled().unwrap_or_default());
                }
                if !state.walk.back_into(&scene) {
                    state.left_off.land(Some(scene.clone()));
                }
            }
            _ => {}
        })
}
