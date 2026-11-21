use std::time::Duration;

use leptos::prelude::*;
use leptos::{IntoView, ev, html};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, MutationObserver, MutationObserverInit, Node, Range};

use crate::feedback::model::PendingPassage;
use crate::feedback::pending::PendingHere;
use crate::icons;
use crate::icons::Icon;
use crate::kept;
use crate::readings;
use crate::speech;

/// The class on the page that puts feedback's marks in the text out of sight, as `::highlight` follows it.
const MARKS_OFF: &str = "feedback-marks-off";
const OFF: &str = "off";
/// The name the writing view paints the passages feedback is pending on under, as a copy's text paints them.
const GIVEN_IN_THE_TEXT: &str = "feedback-given";
/// How long the text rests after a change before the passages are found in it again.
const FOUND_AGAIN_AFTER: Duration = Duration::from_millis(300);

/// Whether feedback is marked in the writing view's text, for its status note to offer the switch.
#[derive(Clone, Copy)]
pub struct MarksWhileWriting(pub RwSignal<bool>);

/// Where whether feedback is marked in each view's text is kept.
pub const KEPT_WHILE_WRITING: &str = "weaveling.writing-feedback-marks";
pub const KEPT_WHILE_READING: &str = "weaveling.reading-feedback-marks";

/// Whether feedback is marked in the text of one view, kept as it was last left there, for whoever reads or writes without it.
/// While shown, the page lets the marks be painted; else it puts them out of sight, and back when the view is left.
pub fn marks_shown(kept_as: &'static str) -> RwSignal<bool> {
    let shown = RwSignal::new(kept::recall(kept_as).as_deref() != Some(OFF));
    Effect::new(move |_| {
        let shown = shown.get();
        if let Some(body) = document().body() {
            _ = body.class_list().toggle_with_force(MARKS_OFF, !shown);
        }
        if shown {
            kept::forget(kept_as);
        } else {
            kept::keep(kept_as, OFF);
        }
    });
    on_cleanup(|| {
        if let Some(body) = document().body() {
            _ = body.class_list().remove_1(MARKS_OFF);
        }
    });

    shown
}

/// The switch for feedback's marks in the text, named, pressed while they are shown.
pub fn marks_switch(shown: RwSignal<bool>) -> impl IntoView {
    html::button()
        .r#type("button")
        .class("marks-switch")
        .attr("aria-label", speech::said("feedback-marks"))
        .attr("title", move || {
            speech::say(if shown.get() {
                "feedback-marks-hide"
            } else {
                "feedback-marks-show"
            })
        })
        .attr("aria-pressed", move || shown.get().to_string())
        .on(ev::click, move |_| shown.update(|shown| *shown = !*shown))
        .child(icons::mark(Icon::Comment))
}

/// Marks the passages feedback is pending on in the writing view's text, each found by its words in its scene as it is now,
/// since the scene may have changed since its copy froze it: in the block it was given on, else anywhere in the scene.
/// Words no longer there are not marked. They are found again whenever the text has rested after a change.
pub fn marked_while_writing(pending: PendingHere) {
    let resting = StoredValue::new_local(None::<TimeoutHandle>);
    let paint = move || {
        let passages = pending.passages();
        let words: Vec<Range> = passages.iter().filter_map(found).collect();
        if words.is_empty() {
            readings::pointing::unhighlighted(GIVEN_IN_THE_TEXT);
        } else {
            readings::pointing::highlighted(GIVEN_IN_THE_TEXT, &words);
        }
    };
    Effect::new(move |_| {
        pending.passages();
        _ = request_animation_frame(paint);
    });
    let changed: Closure<dyn Fn()> = Closure::new(move || {
        if let Some(waiting) = resting.try_update_value(Option::take).flatten() {
            waiting.clear();
        }
        resting.set_value(set_timeout(paint, FOUND_AGAIN_AFTER).ok());
    });
    let watching = MutationObserver::new(changed.as_ref().unchecked_ref()).ok();
    let watched = StoredValue::new_local((watching, Some(changed)));
    _ = request_animation_frame(move || {
        let Some(writing) = document().query_selector(".writing").ok().flatten() else {
            return;
        };
        let options = MutationObserverInit::new();
        options.set_child_list(true);
        options.set_character_data(true);
        options.set_subtree(true);
        watched.with_value(|(watching, _)| {
            if let Some(watching) = watching {
                _ = watching.observe_with_options(&writing, &options);
            }
        });
    });
    on_cleanup(move || {
        if let Some(waiting) = resting.try_update_value(Option::take).flatten() {
            waiting.clear();
        }
        _ = watched.try_update_value(|(watching, changed)| {
            if let Some(watching) = watching.take() {
                watching.disconnect();
            }
            changed.take();
        });
        readings::pointing::unhighlighted(GIVEN_IN_THE_TEXT);
    });
}

/// A passage's words in its scene as it is written now: in the block it was given on, else anywhere in the scene.
fn found(passage: &PendingPassage) -> Option<Range> {
    let text = document()
        .query_selector(&format!(
            ".writing .scene[data-scene=\"{}\"] .ProseMirror",
            passage.scene
        ))
        .ok()??;
    let block = passage
        .block_path
        .first()
        .and_then(|at| text.children().item(*at));

    block
        .and_then(|block| words_in(&block, &passage.quote))
        .or_else(|| words_in(&text, &passage.quote))
}

/// The first stretch of text in an element that says these words, across the text nodes it runs over.
fn words_in(within: &Element, quote: &str) -> Option<Range> {
    let mut texts: Vec<(Node, String)> = Vec::new();
    texts_in(within.as_ref(), &mut texts);
    let whole: String = texts.iter().map(|(_, said)| said.as_str()).collect();
    let starts = whole.find(quote)?;
    let (start, start_at) = position_of(&texts, starts)?;
    let (end, end_at) = position_of(&texts, starts + quote.len())?;
    let words = document().create_range().ok()?;
    words.set_start(&start, start_at).ok()?;
    words.set_end(&end, end_at).ok()?;

    Some(words)
}

fn texts_in(node: &Node, texts: &mut Vec<(Node, String)>) {
    if node.node_type() == Node::TEXT_NODE {
        texts.push((node.clone(), node.text_content().unwrap_or_default()));
        return;
    }
    let children = node.child_nodes();
    for at in 0..children.length() {
        if let Some(child) = children.item(at) {
            texts_in(&child, texts);
        }
    }
}

/// Where the byte so far into the text nodes' joined words lies: its node, and the offset in it as the browser counts, in UTF-16.
fn position_of(texts: &[(Node, String)], bytes: usize) -> Option<(Node, u32)> {
    let mut left = bytes;
    for (node, said) in texts {
        if left <= said.len() {
            let offset = said.get(..left)?.encode_utf16().count();
            return Some((node.clone(), u32::try_from(offset).ok()?));
        }
        left -= said.len();
    }

    None
}
