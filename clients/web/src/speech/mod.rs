mod catalogue;
pub mod language;
pub mod picker;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;

use crate::kept::{keep, recall};
use crate::speech::catalogue::{knows, worded};
use crate::speech::language::{Language, the_reader_s};

const SPOKEN: &str = "weaveling.language";

#[derive(Clone, Copy)]
pub struct Speech {
    spoken: RwSignal<Language>,
}

impl Speech {
    pub fn heard() -> Self {
        let spoken = recall(SPOKEN)
            .and_then(|tag| Language::told(&tag))
            .unwrap_or_else(the_reader_s);
        mark_the_page(spoken);

        Self {
            spoken: RwSignal::new(spoken),
        }
    }

    pub fn language(self) -> Language {
        self.spoken.get()
    }

    pub fn speak(self, language: Language) {
        self.spoken.set(language);
        keep(SPOKEN, language.tag());
        mark_the_page(language);
    }

    pub fn say(self, message: &str) -> String {
        worded(self.spoken.get(), message, None)
    }

    pub fn say_with(self, message: &str, args: &FluentArgs) -> String {
        worded(self.spoken.get(), message, Some(args))
    }

    pub fn knows(self, message: &str) -> bool {
        knows(self.spoken.get(), message)
    }
}

pub fn say(message: &str) -> String {
    worded(spoken_here(), message, None)
}

pub fn say_with(message: &str, args: &FluentArgs) -> String {
    worded(spoken_here(), message, Some(args))
}

pub fn say_about(message: &str, name: &str, value: &str) -> String {
    let mut args = FluentArgs::new();
    args.set(name.to_owned(), value.to_owned());

    worded(spoken_here(), message, Some(&args))
}

fn spoken_here() -> Language {
    use_context::<Speech>().map_or(Language::English, Speech::language)
}

pub fn said(message: &'static str) -> impl Fn() -> String + Copy + Send + Sync + 'static {
    let speech = expect_context::<Speech>();

    move || speech.say(message)
}

fn mark_the_page(language: Language) {
    if let Some(page) = document().document_element() {
        _ = page.set_attribute("lang", language.tag());
    }
}
