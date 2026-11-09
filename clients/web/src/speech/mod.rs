mod catalogue;
pub mod language;
pub mod picker;

use fluent_bundle::FluentArgs;
use leptos::prelude::*;
use leptos::task;

use crate::kept;
use crate::speech::language::Language;

const SPOKEN: &str = "weaveling.language";

#[derive(Clone, Copy)]
pub struct Speech {
    spoken: RwSignal<Language>,
}

/// Fetches the catalogue of the language the page will be spoken in, before anything is shown in it.
pub async fn readied() {
    catalogue::fetched(first_spoken()).await;
}

fn first_spoken() -> Language {
    kept::recall(SPOKEN)
        .and_then(|tag| Language::told(&tag))
        .unwrap_or_else(crate::speech::language::the_reader_s)
}

impl Speech {
    pub fn heard() -> Self {
        let spoken = first_spoken();
        mark_the_page(spoken);

        Self {
            spoken: RwSignal::new(spoken),
        }
    }

    pub fn language(self) -> Language {
        self.spoken.get()
    }

    /// Speaks the language once its catalogue is fetched; where it cannot be, the one spoken stays.
    pub fn speak(self, language: Language) {
        task::spawn_local(async move {
            if !catalogue::fetched(language).await {
                return;
            }
            self.spoken.set(language);
            kept::keep(SPOKEN, language.tag());
            mark_the_page(language);
        });
    }

    pub fn say(self, message: &str) -> String {
        crate::speech::catalogue::worded(self.spoken.get(), message, None)
    }

    pub fn say_with(self, message: &str, args: &FluentArgs) -> String {
        crate::speech::catalogue::worded(self.spoken.get(), message, Some(args))
    }

    pub fn knows(self, message: &str) -> bool {
        catalogue::knows(self.spoken.get(), message)
    }
}

pub fn say(message: &str) -> String {
    crate::speech::catalogue::worded(spoken_here(), message, None)
}

pub fn say_with(message: &str, args: &FluentArgs) -> String {
    crate::speech::catalogue::worded(spoken_here(), message, Some(args))
}

pub fn say_about(message: &str, name: &str, value: &str) -> String {
    let mut args = FluentArgs::new();
    args.set(name.to_owned(), value.to_owned());

    crate::speech::catalogue::worded(spoken_here(), message, Some(&args))
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
