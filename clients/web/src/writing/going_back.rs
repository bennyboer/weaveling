use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};
use crate::writing::write_view_state::WriteViewState;

pub fn going_back(state: WriteViewState) -> impl IntoView {
    move || {
        state.left_off.offered().map(|scene| {
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
