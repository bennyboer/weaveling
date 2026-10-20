use leptos::prelude::window;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    English,
    German,
    Spanish,
    French,
    Portuguese,
}

impl Language {
    pub const EVERY: [Self; 5] = [
        Self::English,
        Self::German,
        Self::Spanish,
        Self::French,
        Self::Portuguese,
    ];

    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::German => "de",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::Portuguese => "pt-BR",
        }
    }

    pub fn own_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::German => "Deutsch",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::Portuguese => "Português do Brasil",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::English => "EN",
            Self::German => "DE",
            Self::Spanish => "ES",
            Self::French => "FR",
            Self::Portuguese => "PT",
        }
    }

    pub fn machine_translated(self) -> bool {
        matches!(self, Self::Spanish | Self::French | Self::Portuguese)
    }

    pub fn told(tag: &str) -> Option<Self> {
        let primary = tag.split('-').next()?.to_ascii_lowercase();

        Self::EVERY
            .into_iter()
            .find(|language| language.tag().split('-').next() == Some(primary.as_str()))
    }
}

pub fn the_reader_s() -> Language {
    window()
        .navigator()
        .languages()
        .iter()
        .filter_map(|tag| tag.as_string())
        .find_map(|tag| Language::told(&tag))
        .unwrap_or(Language::English)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_told_by_its_language_whatever_its_region() {
        assert_eq!(Language::told("de-AT"), Some(Language::German));
        assert_eq!(Language::told("pt-PT"), Some(Language::Portuguese));
        assert_eq!(Language::told("pt-BR"), Some(Language::Portuguese));
        assert_eq!(Language::told("EN"), Some(Language::English));
        assert_eq!(Language::told("fr"), Some(Language::French));
        assert_eq!(Language::told("es-MX"), Some(Language::Spanish));
    }

    #[test]
    fn a_language_the_app_does_not_speak_is_told_by_none() {
        assert_eq!(Language::told("it"), None);
        assert_eq!(Language::told(""), None);
    }
}
