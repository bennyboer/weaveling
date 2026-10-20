use leptos::prelude::window;

pub const BOOK_LANGUAGES: &[&str] = &[
    "de", "en", "fr", "es", "it", "nl", "pt", "pl", "sv", "da", "nb", "fi", "cs",
];

pub fn the_browser_s() -> Option<&'static str> {
    let spoken = window().navigator().language()?;
    let primary = spoken.split('-').next()?.to_ascii_lowercase();

    BOOK_LANGUAGES.iter().copied().find(|tag| *tag == primary)
}
