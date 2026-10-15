use std::time::Duration;

use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons::{Icon, mark};
use crate::writing::side::Side;
use crate::writing::word_count::Counted;
use crate::writing::write_view_state::WriteViewState;

const SETTLE: Duration = Duration::from_millis(500);

pub fn status_note(state: WriteViewState) -> impl IntoView {
    let shown_saving = RwSignal::new(false);
    let settling = StoredValue::new(None::<TimeoutHandle>);

    Effect::new(move || {
        let saving = state.saving.is_saving();

        if let Some(handle) = settling.get_value() {
            handle.clear();
        }
        settling.set_value(None);

        if saving {
            shown_saving.set(true);
        } else {
            settling.set_value(set_timeout(move || _ = shown_saving.try_set(false), SETTLE).ok());
        }
    });
    on_cleanup(move || {
        if let Some(handle) = settling.get_value() {
            handle.clear();
        }
    });

    html::div().class("status-note").child((
        match state.side {
            Side::Book => html::button()
                .r#type("button")
                .class("word-count")
                .attr("title", move || match state.word_count.counted() {
                    Counted::Section => "Count the whole book",
                    Counted::Book => "Count the section being written",
                })
                .on(ev::click, move |_| state.word_count.toggle())
                .child(move || counted(state))
                .into_any(),
            Side::Loose => html::span()
                .class("word-count")
                .child(move || {
                    let words = state.word_count.words_in(&state.loose.in_order());

                    format!("{} not in the book", in_words(words))
                })
                .into_any(),
        },
        html::span().class("save-state").child(move || {
            if shown_saving.get() {
                (mark(Icon::Saving), html::span().child("Saving…"))
            } else {
                (mark(Icon::Saved), html::span().child("All saved"))
            }
        }),
    ))
}

fn counted(state: WriteViewState) -> String {
    let words = in_words(state.word_count.words());

    match state.word_count.counted() {
        Counted::Book => format!("{words} in the book"),
        Counted::Section => match state
            .word_count
            .section()
            .and_then(|section| state.structure.title_of(&section))
            .filter(|title| !title.trim().is_empty())
        {
            Some(title) => format!("{words} in “{title}”"),
            None => format!("{words} in this section"),
        },
    }
}

fn in_words(count: usize) -> String {
    let digits = count.to_string();
    let grouped = digits
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join(",");

    match count {
        1 => "1 word".to_owned(),
        _ => format!("{grouped} words"),
    }
}
