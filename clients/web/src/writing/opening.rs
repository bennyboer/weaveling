use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::outline::model::SectionId;
use crate::writing::heading::walk_on;
use crate::writing::walk::Stop;
use crate::writing::write_view_state::WriteViewState;

const INVITATION: &str = "An empty scene. Start writing\u{2026}";

pub fn opening(section: SectionId, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let stop = Stop::Opening(section.clone());

    Effect::new({
        let stop = stop.clone();

        move || {
            if let Some(shown) = host.get() {
                state.walk.line_at(stop.clone(), (*shown).clone());
            }
        }
    });
    on_cleanup({
        let stop = stop.clone();

        move || state.walk.line_gone(&stop)
    });

    html::div()
        .class("opening")
        .attr("contenteditable", "plaintext-only")
        .attr("role", "textbox")
        .attr("aria-label", "A new scene")
        .attr("data-placeholder", INVITATION)
        .node_ref(host)
        .on(ev::input, {
            let section = section.clone();

            move |_| {
                if let Some(shown) = host.get_untracked() {
                    state.beginnings.opening_words_are(
                        section.clone(),
                        shown.text_content().unwrap_or_default(),
                    );
                }
                state.beginnings.begin_scene(section.clone());
            }
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Enter" {
                event.prevent_default();
            }
            walk_on(&event, host, stop.clone(), state);
        })
}
