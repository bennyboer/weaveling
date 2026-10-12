use std::time::Duration;

use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use web_sys::{HtmlDivElement, KeyboardEvent, Node};

use crate::outline::model::{Section, SectionId};
use crate::writing::write_state::{Stop, Towards, WriteState};

const SAVE_AFTER: Duration = Duration::from_millis(700);

pub const UNTITLED: &str = "Untitled section";

const FIRST: &str = "Your book's first section";

pub fn section_heading(depth: usize, section: Section, state: WriteState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let id = section.id.clone();
    let written = match section.title.as_str() {
        "" if state.takes_focus(&id) => {
            let carried = state.first_words();
            focus_when_mounted(host);

            carried
        }
        titled => titled.to_owned(),
    };
    let saved = StoredValue::new(written.clone());
    let waiting = StoredValue::new(None::<TimeoutHandle>);

    let save = move |id: SectionId| {
        if let Some(handle) = waiting.get_value() {
            handle.clear();
        }
        waiting.set_value(None);

        let Some(shown) = host.get_untracked() else {
            return;
        };
        let title = shown.text_content().unwrap_or_default().trim().to_owned();

        if saved.with_value(|before| before != &title) {
            saved.set_value(title.clone());
            state.retitle(id, title);
        }
    };

    let level = depth.min(2) + 2;

    Effect::new({
        let id = id.clone();

        move || {
            if let Some(shown) = host.get() {
                state.heading_at(id.clone(), (*shown).clone());
            }
        }
    });
    on_cleanup({
        let id = id.clone();

        move || state.heading_gone(&id)
    });

    html::div()
        .class(format!("heading level-{level}"))
        .role("heading")
        .attr("aria-level", level.to_string())
        .attr("contenteditable", "plaintext-only")
        .attr("spellcheck", "false")
        .attr("data-placeholder", UNTITLED)
        .node_ref(host)
        .child(written)
        .on(ev::input, {
            let id = id.clone();

            move |_| {
                if let Some(handle) = waiting.get_value() {
                    handle.clear();
                }
                let id = id.clone();
                waiting.set_value(set_timeout(move || save(id), SAVE_AFTER).ok());
            }
        })
        .on(ev::blur, {
            let id = id.clone();

            move |_| save(id.clone())
        })
        .on(ev::keydown, move |event| {
            leave_on_enter(&event, host);
            walk_on(&event, host, &id, state);
        })
}

fn walk_on(event: &KeyboardEvent, host: NodeRef<Div>, section: &SectionId, state: WriteState) {
    let Some(shown) = host.get_untracked() else {
        return;
    };
    let (at_start, at_end) = caret_at_edges(&shown);
    let towards = match event.key().as_str() {
        "ArrowUp" => Towards::Before,
        "ArrowDown" => Towards::After,
        "ArrowLeft" if at_start => Towards::Before,
        "ArrowRight" if at_end => Towards::After,
        _ => return,
    };

    if state.leave(&Stop::Heading(section.clone()), towards) {
        event.prevent_default();
    }
}

fn caret_at_edges(shown: &HtmlDivElement) -> (bool, bool) {
    let Some(chosen) = window().get_selection().ok().flatten() else {
        return (false, false);
    };
    if !chosen.is_collapsed() {
        return (false, false);
    }
    let Some(at) = chosen.focus_node() else {
        return (false, false);
    };
    let offset = chosen.focus_offset();
    let whole = shown
        .text_content()
        .unwrap_or_default()
        .encode_utf16()
        .count() as u32;
    let own: &Node = shown;

    if at.is_same_node(Some(own)) {
        return (offset == 0, whole == 0 || offset > 0);
    }

    (offset == 0, offset >= whole)
}

pub fn first_heading(state: WriteState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();

    html::div()
        .class("heading level-2")
        .role("heading")
        .attr("aria-level", "2")
        .attr("contenteditable", "plaintext-only")
        .attr("spellcheck", "false")
        .attr("data-placeholder", FIRST)
        .node_ref(host)
        .on(ev::input, move |_| {
            if let Some(shown) = host.get_untracked() {
                state.first_words_are(shown.text_content().unwrap_or_default());
            }
            state.begin();
        })
        .on(ev::keydown, move |event| leave_on_enter(&event, host))
}

fn leave_on_enter(event: &KeyboardEvent, host: NodeRef<Div>) {
    if event.key() == "Enter" {
        event.prevent_default();

        if let Some(shown) = host.get_untracked() {
            let _ = shown.blur();
        }
    }
}

fn focus_when_mounted(host: NodeRef<Div>) {
    Effect::new(move || {
        let Some(shown) = host.get() else {
            return;
        };
        let _ = shown.focus();

        if let Some(chosen) = window().get_selection().ok().flatten() {
            let _ = chosen.select_all_children(&shown);
            let _ = chosen.collapse_to_end();
        }
    });
}
