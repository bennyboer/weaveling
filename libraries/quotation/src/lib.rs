use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuotationMarks {
    PointingIn,
    LowHigh,
    Curly,
    SpacedOut,
    PointingOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidQuotationMarks {
    #[error("{0:?} is not a quotation style; one of {styles}", styles = QuotationMarks::listed())]
    Unknown(String),
}

impl QuotationMarks {
    pub const ALL: [Self; 5] = [
        Self::PointingIn,
        Self::LowHigh,
        Self::Curly,
        Self::SpacedOut,
        Self::PointingOut,
    ];

    pub fn new(raw: &str) -> Result<Self, InvalidQuotationMarks> {
        Self::named(raw.trim()).ok_or_else(|| InvalidQuotationMarks::Unknown(raw.to_owned()))
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|style| style.name() == name)
    }

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

    pub fn name(self) -> &'static str {
        match self {
            Self::PointingIn => "pointing-in",
            Self::LowHigh => "low-high",
            Self::Curly => "curly",
            Self::SpacedOut => "spaced-out",
            Self::PointingOut => "pointing-out",
        }
    }

    pub fn opening(self) -> &'static str {
        match self {
            Self::PointingIn => "»",
            Self::LowHigh => "„",
            Self::Curly => "“",
            Self::SpacedOut => "«\u{202F}",
            Self::PointingOut => "«",
        }
    }

    pub fn closing(self) -> &'static str {
        match self {
            Self::PointingIn => "«",
            Self::LowHigh => "“",
            Self::Curly => "”",
            Self::SpacedOut => "\u{202F}»",
            Self::PointingOut => "»",
        }
    }

    pub fn shown(self) -> String {
        format!("{}…{}", self.opening(), self.closing())
    }

    fn listed() -> String {
        Self::ALL.map(Self::name).join(", ")
    }
}

impl Display for QuotationMarks {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
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
            assert_eq!(QuotationMarks::new(style.name()), Ok(style));
        }
        assert_eq!(QuotationMarks::named("guillemets"), None);
    }

    #[test]
    fn a_name_is_read_with_the_spaces_around_it_left_out() {
        assert_eq!(
            QuotationMarks::new(" low-high "),
            Ok(QuotationMarks::LowHigh)
        );
        assert_eq!(
            QuotationMarks::new("guillemets"),
            Err(InvalidQuotationMarks::Unknown("guillemets".to_owned()))
        );
    }

    #[test]
    fn french_marks_keep_a_narrow_space_inside() {
        assert_eq!(QuotationMarks::SpacedOut.shown(), "«\u{202F}…\u{202F}»");
        assert_eq!(QuotationMarks::PointingIn.shown(), "»…«");
    }
}
