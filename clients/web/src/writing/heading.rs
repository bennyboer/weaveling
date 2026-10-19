use std::time::Duration;

use leptos::html::Div;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use web_sys::{HtmlDivElement, KeyboardEvent, Node};

use crate::outline::model::{Section, SectionId};
use crate::writing::asking::Asking;
use crate::writing::beginnings::Pending;
use crate::writing::walk::{Stop, Towards};
use crate::writing::write_view_state::WriteViewState;

const SAVE_AFTER: Duration = Duration::from_millis(700);

pub const UNTITLED: &str = "Untitled section";

const FIRST: &str = "Your book's first section";

const NEXT: &str = "A new section";

pub fn level_of(depth: usize) -> usize {
    depth.min(2) + 2
}

pub fn nest(section: SectionId, state: WriteViewState) {
    if state.structure.can_nest(&section) {
        state.walk.arrive_when_drawn(Stop::Heading(section.clone()));
        state.structure.nest(section);
    }
}

pub fn lift(section: SectionId, state: WriteViewState) {
    if state.structure.can_lift(&section) {
        state.walk.arrive_when_drawn(Stop::Heading(section.clone()));
        state.structure.lift(section);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptTitle {
    saved: StoredValue<String>,
    basis: StoredValue<String>,
}

impl KeptTitle {
    pub fn take(self, theirs: String) {
        _ = self.saved.try_set_value(theirs.clone());
        _ = self.basis.try_set_value(theirs);
    }

    pub fn keep(self, mine: String, theirs: String) {
        _ = self.saved.try_set_value(mine);
        _ = self.basis.try_set_value(theirs);
    }
}

pub fn section_heading(depth: usize, section: Section, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let id = section.id.clone();
    let written = match section.title.as_str() {
        "" if state.beginnings.takes_focus(&id) => {
            let carried = state.beginnings.section_words();
            focus_when_mounted(host);

            carried
        }
        titled => titled.to_owned(),
    };
    let kept = KeptTitle {
        saved: StoredValue::new(written.clone()),
        basis: StoredValue::new(section.title.clone()),
    };
    let waiting = StoredValue::new(None::<TimeoutHandle>);
    let heard = StoredValue::new(section.title.clone());

    let save = move |id: SectionId| {
        if let Some(handle) = waiting.get_value() {
            handle.clear();
            state.saving.write_done();
        }
        waiting.set_value(None);

        let Some(shown) = host.get_untracked() else {
            return;
        };
        let mine = shown.text_content().unwrap_or_default().trim().to_owned();
        let theirs = heard.get_value();

        if kept.basis.with_value(|basis| basis != &theirs) {
            if kept.saved.with_value(|saved| saved == &mine) || mine == theirs {
                kept.take(theirs.clone());
                shown.set_text_content(Some(&theirs));
            } else {
                state.margin.ask(Asking::RenamedMeanwhile {
                    section: id,
                    mine,
                    theirs,
                    kept,
                });
            }
            return;
        }
        if kept.saved.with_value(|saved| saved != &mine) {
            kept.saved.set_value(mine.clone());
            state.structure.retitle(id, mine);
        }
    };

    let level = level_of(depth);

    Effect::new({
        let id = id.clone();

        move || {
            let (Some(title), Some(shown)) = (state.structure.title_of(&id), host.get()) else {
                return;
            };
            if heard.with_value(|before| before == &title) {
                return;
            }
            heard.set_value(title.clone());
            if kept.saved.with_value(|saved| saved == &title) {
                kept.basis.set_value(title);
                return;
            }
            let editing =
                shown.matches(":focus").unwrap_or(false) || waiting.with_value(Option::is_some);
            if !editing {
                kept.take(title.clone());
                shown.set_text_content(Some(&title));
            }
        }
    });

    Effect::new({
        let id = id.clone();

        move || {
            if let Some(shown) = host.get() {
                state
                    .walk
                    .line_at(Stop::Heading(id.clone()), (*shown).clone());
            }
        }
    });
    on_cleanup({
        let id = id.clone();

        move || {
            state.walk.line_gone(&Stop::Heading(id.clone()));
            if let Some(handle) = waiting.get_value() {
                handle.clear();
                state.saving.write_done();
            }
        }
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
                match waiting.get_value() {
                    Some(handle) => handle.clear(),
                    None => state.saving.write_begun(),
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
            if event.key() == "Tab" {
                event.prevent_default();
                save(id.clone());

                if event.shift_key() {
                    lift(id.clone(), state);
                } else {
                    nest(id.clone(), state);
                }

                return;
            }
            if event.key() == "Enter" {
                event.prevent_default();

                if !state.walk.leave(&Stop::Heading(id.clone()), Towards::After)
                    && let Some(shown) = host.get_untracked()
                {
                    let _ = shown.blur();
                }
            }
            walk_on(&event, host, Stop::Heading(id.clone()), state);
        })
}

pub fn walk_on(event: &KeyboardEvent, host: NodeRef<Div>, from: Stop, state: WriteViewState) {
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

    if state.walk.leave(&from, towards) {
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

pub fn first_heading(state: WriteViewState) -> impl IntoView {
    new_heading(None, 0, state)
}

pub fn pending_heading(after: SectionId, depth: usize, state: WriteViewState) -> impl IntoView {
    new_heading(Some(after), depth, state)
}

fn new_heading(after: Option<SectionId>, depth: usize, state: WriteViewState) -> impl IntoView {
    let host: NodeRef<Div> = NodeRef::new();
    let level = level_of(depth);
    let pending = after.clone().map(|after| Pending::Section { after });

    if pending.is_some() {
        focus_when_mounted(host);
    }

    html::div()
        .class(format!("heading level-{level} new"))
        .role("heading")
        .attr("aria-level", level.to_string())
        .attr("contenteditable", "plaintext-only")
        .attr("spellcheck", "false")
        .attr(
            "data-placeholder",
            if after.is_some() { NEXT } else { FIRST },
        )
        .node_ref(host)
        .on(ev::input, {
            let after = after.clone();

            move |_| {
                if let Some(shown) = host.get_untracked() {
                    state
                        .beginnings
                        .section_words_are(shown.text_content().unwrap_or_default());
                }
                state.beginnings.begin_section(after.clone());
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
            }
            let Some(after) = &after else {
                return;
            };
            let towards = match event.key().as_str() {
                "ArrowUp" => Towards::Before,
                "ArrowDown" => Towards::After,
                _ => return,
            };
            let Some(last) = state
                .structure
                .end_of_block(after)
                .and_then(|end| state.walk.last_stop_in(&end))
            else {
                return;
            };
            let moved = match towards {
                Towards::Before => state.walk.arrive(&last, Towards::Before),
                Towards::After => state.walk.leave(&last, Towards::After),
            };

            if moved {
                event.prevent_default();
            }
        })
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
