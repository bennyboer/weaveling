use std::time::Duration;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::{IntoView, ev, html};

use crate::icons;
use crate::icons::Icon;
use crate::speech;
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
        html::button()
            .r#type("button")
            .class("enter-focus")
            .attr("aria-label", speech::said("status-focus"))
            .attr("title", speech::said("status-focus-hint"))
            .attr("aria-pressed", move || state.focus.is_on().to_string())
            .on(ev::click, move |_| state.focus.toggle())
            .child(icons::mark(Icon::Focus)),
        html::button()
            .r#type("button")
            .class("open-settings")
            .attr("aria-label", speech::said("writing-settings"))
            .attr("title", speech::said("writing-settings"))
            .on(ev::click, move |_| state.settings.open())
            .child(icons::mark(Icon::Settings)),
        match state.side {
            Side::Book => html::button()
                .r#type("button")
                .class("word-count")
                .attr("title", move || match state.word_count.counted() {
                    Counted::Section => speech::say("count-whole-book"),
                    Counted::Book => speech::say("count-section"),
                })
                .on(ev::click, move |_| state.word_count.toggle())
                .child(move || counted(state))
                .into_any(),
            Side::Loose => html::span()
                .class("word-count")
                .child(move || {
                    let words = state.word_count.words_in(&state.loose.in_order());

                    speech::say_about("words-not-in-the-book", "words", &in_words(words))
                })
                .into_any(),
        },
        html::span().class("save-state").child(move || {
            if shown_saving.get() {
                (
                    icons::mark(Icon::Saving),
                    html::span().child(speech::said("saving")),
                )
            } else {
                (
                    icons::mark(Icon::Saved),
                    html::span().child(speech::said("all-saved")),
                )
            }
        }),
    ))
}

fn counted(state: WriteViewState) -> String {
    let words = in_words(state.word_count.words());

    match state.word_count.counted() {
        Counted::Book => speech::say_about("words-in-the-book", "words", &words),
        Counted::Section => match state
            .writing_in
            .section()
            .and_then(|section| state.structure.title_of(&section))
            .filter(|title| !title.trim().is_empty())
        {
            Some(title) => {
                let mut args = FluentArgs::new();
                args.set("words", words);
                args.set("section", title);

                speech::say_with("words-in-section", &args)
            }
            None => speech::say_about("words-in-this-section", "words", &words),
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
        .join(&speech::say("digit-group"));
    let mut args = FluentArgs::new();
    args.set("count", count);
    args.set("shown", grouped);

    speech::say_with("words", &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_word_is_told_as_one() {
        assert_eq!(in_words(1), "1 word");
    }

    #[test]
    fn many_words_are_grouped_by_thousands() {
        assert_eq!(in_words(0), "0 words");
        assert_eq!(in_words(999), "999 words");
        assert_eq!(in_words(1_234_567), "1,234,567 words");
    }
}
