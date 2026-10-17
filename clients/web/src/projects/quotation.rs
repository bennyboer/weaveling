const NARROW_SPACE: &str = "\u{202F}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotationMarks {
    PointingIn,
    LowHigh,
    Curly,
    SpacedOut,
    PointingOut,
}

impl QuotationMarks {
    pub const ALL: [Self; 5] = [
        Self::PointingIn,
        Self::LowHigh,
        Self::Curly,
        Self::SpacedOut,
        Self::PointingOut,
    ];

    pub fn of_language(language: Option<&str>) -> Self {
        let primary = language
            .and_then(|tag| tag.split('-').next())
            .map(str::to_ascii_lowercase);

        match primary.as_deref() {
            Some("de" | "da") => Self::PointingIn,
            Some("pl" | "cs") => Self::LowHigh,
            Some("fr") => Self::SpacedOut,
            Some("es" | "it" | "pt" | "nb") => Self::PointingOut,
            _ => Self::Curly,
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|style| style.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::PointingIn => "pointing-in",
            Self::LowHigh => "low-high",
            Self::Curly => "curly",
            Self::SpacedOut => "spaced-out",
            Self::PointingOut => "pointing-out",
        }
    }

    pub fn opening(self) -> String {
        match self {
            Self::PointingIn => "»".to_owned(),
            Self::LowHigh => "„".to_owned(),
            Self::Curly => "“".to_owned(),
            Self::SpacedOut => format!("«{NARROW_SPACE}"),
            Self::PointingOut => "«".to_owned(),
        }
    }

    pub fn closing(self) -> String {
        match self {
            Self::PointingIn => "«".to_owned(),
            Self::LowHigh => "“".to_owned(),
            Self::Curly => "”".to_owned(),
            Self::SpacedOut => format!("{NARROW_SPACE}»"),
            Self::PointingOut => "»".to_owned(),
        }
    }

    pub fn shown(self) -> String {
        format!("{}…{}", self.opening(), self.closing())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_book_s_language_brings_its_house_style() {
        assert_eq!(
            QuotationMarks::of_language(Some("de")),
            QuotationMarks::PointingIn
        );
        assert_eq!(
            QuotationMarks::of_language(Some("en-GB")),
            QuotationMarks::Curly
        );
        assert_eq!(
            QuotationMarks::of_language(Some("fr")),
            QuotationMarks::SpacedOut
        );
        assert_eq!(
            QuotationMarks::of_language(Some("it")),
            QuotationMarks::PointingOut
        );
        assert_eq!(
            QuotationMarks::of_language(Some("cs")),
            QuotationMarks::LowHigh
        );
        assert_eq!(
            QuotationMarks::of_language(None),
            QuotationMarks::Curly,
            "a book with no language yet quotes as English does"
        );
    }

    #[test]
    fn every_style_is_known_by_its_name() {
        for style in QuotationMarks::ALL {
            assert_eq!(QuotationMarks::named(style.name()), Some(style));
        }
        assert_eq!(QuotationMarks::named("guillemets"), None);
    }

    #[test]
    fn french_marks_keep_a_narrow_space_inside() {
        assert_eq!(QuotationMarks::SpacedOut.shown(), "«\u{202F}…\u{202F}»");
        assert_eq!(QuotationMarks::PointingIn.shown(), "»…«");
    }
}
