use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};
use crate::scenes::model::SceneId;
use crate::writing::viewport::SCENE_PLACE;
use crate::writing::write_view_state::WriteViewState;

pub fn going_back(state: WriteViewState) -> impl IntoView {
    let in_sight = RwSignal::new(false);
    let look = move || {
        let seen = state
            .left_off
            .offered_untracked()
            .is_some_and(|scene| is_in_sight(&scene));

        if in_sight.get_untracked() != seen {
            in_sight.set(seen);
        }
    };

    Effect::new(move || {
        if state.left_off.offered().is_some() {
            _ = request_animation_frame(look);
        }
    });
    let scrolling = window_event_listener(ev::scroll, move |_| look());
    let resizing = window_event_listener(ev::resize, move |_| look());
    on_cleanup(move || {
        scrolling.remove();
        resizing.remove();
    });

    move || {
        state
            .left_off
            .offered()
            .filter(|_| !in_sight.get())
            .map(|scene| {
                let named = move || {
                    let label = state.labels.of_scene(&scene);

                    match label.shown.trim() {
                        "" => "An empty scene".to_owned(),
                        _ => label.shown,
                    }
                };

                html::button()
                    .r#type("button")
                    .class("going-back")
                    .on(ev::click, move |_| state.left_off.go_back())
                    .child((
                        mark(Icon::Later),
                        html::span().child("Back to where you left off"),
                        html::span().class("where").child(named),
                    ))
            })
    }
}

fn is_in_sight(scene: &SceneId) -> bool {
    let Ok(Some(place)) = document().query_selector(&format!("[{SCENE_PLACE}=\"{scene}\"]")) else {
        return false;
    };
    let shown = place.get_bounding_client_rect();
    let height = window()
        .inner_height()
        .ok()
        .and_then(|height| height.as_f64())
        .unwrap_or_default();

    shown.bottom() > 0.0 && shown.top() < height
}
