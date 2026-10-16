use leptos::prelude::window;

pub const BOOK_LANGUAGES: &[(&str, &str)] = &[
    ("de", "German (Deutsch)"),
    ("en", "English"),
    ("fr", "French (Français)"),
    ("es", "Spanish (Español)"),
    ("it", "Italian (Italiano)"),
    ("nl", "Dutch (Nederlands)"),
    ("pt", "Portuguese (Português)"),
    ("pl", "Polish (Polski)"),
    ("sv", "Swedish (Svenska)"),
    ("da", "Danish (Dansk)"),
    ("nb", "Norwegian (Norsk)"),
    ("fi", "Finnish (Suomi)"),
    ("cs", "Czech (Čeština)"),
];

pub fn the_browser_s() -> Option<&'static str> {
    let spoken = window().navigator().language()?;
    let primary = spoken.split('-').next()?.to_ascii_lowercase();

    BOOK_LANGUAGES
        .iter()
        .map(|(tag, _)| *tag)
        .find(|tag| *tag == primary)
}
