use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;
use crate::writing::beginnings::Pending;
use crate::writing::heading::walk_on;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

const INVITATION: &str = "An empty scene. Start writing\u{2026}";

const NEXT: &str = "A new scene";

#[derive(Clone)]
enum Line {
    Opening,
    After(SceneId),
    First,
}

pub fn opening(section: SectionId, state: WriteViewState) -> impl IntoView {
    scene_line(section, Line::Opening, state)
}

pub fn pending_scene(section: SectionId, after: SceneId, state: WriteViewState) -> impl IntoView {
    scene_line(section, Line::After(after), state)
}

pub fn pending_first_scene(section: SectionId, state: WriteViewState) -> impl IntoView {
    scene_line(section, Line::First, state)
}

fn scene_line(section: SectionId, line: Line, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let stop = Stop::Opening(section.clone());
    let after = match &line {
        Line::After(scene) => Some(scene.clone()),
        Line::Opening | Line::First => None,
    };
    let pending = match &line {
        Line::Opening => None,
        Line::After(_) | Line::First => Some(Pending::Scene {
            section: section.clone(),
            after: after.clone(),
        }),
    };

    match &pending {
        Some(_) => focus_when_mounted(host),
        None => {
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
        }
    }

    html::div()
        .class("opening")
        .attr("contenteditable", "plaintext-only")
        .attr("role", "textbox")
        .attr("aria-label", NEXT)
        .attr("data-placeholder", INVITATION)
        .node_ref(host)
        .on(ev::input, {
            let section = section.clone();
            let after = after.clone();

            move |_| {
                if let Some(shown) = host.get_untracked() {
                    state.beginnings.opening_words_are(
                        section.clone(),
                        shown.text_content().unwrap_or_default(),
                    );
                }
                state.beginnings.begin_scene(section.clone(), after.clone());
            }
        })
        .on(ev::blur, {
            let pending = pending.clone();

            move |_| {
                let empty = host
                    .get_untracked()
                    .is_none_or(|shown| shown.text_content().unwrap_or_default().is_empty());

                if let (true, Some(pending)) = (empty, &pending) {
                    state.beginnings.drop_pending(pending);
                }
            }
        })
        .on(ev::keydown, move |event| {
            if event.key() == "Enter" {
                event.prevent_default();
                state.beginnings.start_pending_section_after(&section);

                return;
            }

            let anchor = match &line {
                Line::Opening => return walk_on(&event, host, stop.clone(), state),
                Line::After(scene) => Stop::Scene(scene.clone()),
                Line::First => Stop::Heading(section.clone()),
            };
            let moved = match event.key().as_str() {
                "ArrowUp" => state.walk.arrive(&anchor, Towards::Before),
                "ArrowDown" => state.walk.leave(&anchor, Towards::After),
                _ => false,
            };

            if moved {
                event.prevent_default();
            }
        })
}

fn focus_when_mounted(host: NodeRef<Div>) {
    Effect::new(move || {
        if let Some(shown) = host.get() {
            let _ = shown.focus();
        }
    });
}
