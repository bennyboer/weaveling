use std::fmt::{self, Display, Formatter};
use std::iter;

use thiserror::Error;
use words::{TextRules, Unfit};

pub use quotation::{InvalidQuotationMarks, QuotationMarks};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BookLanguage(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidBookLanguage {
    #[error("{0:?} is not a language tag such as \"de\", \"en\" or \"en-GB\"")]
    NotATag(String),
}

impl BookLanguage {
    pub const MAX_CHARS: usize = 35;

    pub fn new(raw: &str) -> Result<Self, InvalidBookLanguage> {
        let refused = || InvalidBookLanguage::NotATag(raw.to_owned());
        let trimmed = raw.trim();

        if trimmed.chars().count() > Self::MAX_CHARS {
            return Err(refused());
        }

        let mut parts = trimmed.split('-');
        let primary = parts.next().unwrap_or_default();
        let primary_fits = (2..=3).contains(&primary.len())
            && primary.chars().all(|letter| letter.is_ascii_alphabetic());
        let rest_fits = parts.clone().all(|part| {
            (1..=8).contains(&part.len()) && part.chars().all(|sign| sign.is_ascii_alphanumeric())
        });

        if !primary_fits || !rest_fits {
            return Err(refused());
        }

        let normalised = iter::once(primary.to_ascii_lowercase())
            .chain(parts.map(|part| match part.len() {
                2 if part.chars().all(|sign| sign.is_ascii_alphabetic()) => {
                    part.to_ascii_uppercase()
                }
                4 if part.chars().all(|sign| sign.is_ascii_alphabetic()) => {
                    part[..1].to_ascii_uppercase() + &part[1..].to_ascii_lowercase()
                }
                _ => part.to_ascii_lowercase(),
            }))
            .collect::<Vec<_>>()
            .join("-");

        Ok(Self(normalised))
    }
}

impl Display for BookLanguage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ProjectDescription(String);

pub type InvalidProjectDescription = Unfit;

impl ProjectDescription {
    const RULES: TextRules = TextRules {
        max_chars: 4_096,
        may_be_blank: true,
        may_break_lines: true,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidProjectDescription> {
        Self::RULES.fitted(raw).map(Self)
    }
}

impl Display for ProjectDescription {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectDetails {
    pub language: Option<BookLanguage>,
    pub quotation: Option<QuotationMarks>,
    pub description: ProjectDescription,
}

impl ProjectDetails {
    pub fn quotation_marks(&self) -> QuotationMarks {
        self.quotation.unwrap_or_else(|| {
            QuotationMarks::of_language(self.language.as_ref().map(|language| language.0.as_str()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_language_tag_is_kept_in_its_usual_case() {
        assert_eq!(
            BookLanguage::new("de").map(|it| it.to_string()),
            Ok("de".to_owned())
        );
        assert_eq!(
            BookLanguage::new(" EN-gb ").map(|it| it.to_string()),
            Ok("en-GB".to_owned()),
            "the language lower case and the region upper, as `lang` expects them"
        );
        assert_eq!(
            BookLanguage::new("sr-Latn-RS").map(|it| it.to_string()),
            Ok("sr-Latn-RS".to_owned()),
            "and a script in title case"
        );
    }

    #[test]
    fn what_is_not_a_language_tag_is_refused() {
        for refused in ["", "d", "deutsch", "de_DE", "en-", "12", "en-toolongpart"] {
            assert!(
                BookLanguage::new(refused).is_err(),
                "{refused:?} should not pass as a language"
            );
        }
    }

    #[test]
    fn a_description_keeps_its_line_breaks_and_loses_its_margins() {
        assert_eq!(
            ProjectDescription::new("  A weaver's daughter.\nA war.  ").map(|it| it.to_string()),
            Ok("A weaver's daughter.\nA war.".to_owned())
        );
    }

    #[test]
    fn a_description_refuses_control_characters_and_great_length() {
        assert_eq!(
            ProjectDescription::new("a\u{7}b"),
            Err(InvalidProjectDescription::ControlCharacter)
        );
        assert_eq!(
            ProjectDescription::new(&"a".repeat(4_097)),
            Err(InvalidProjectDescription::TooLong {
                max: 4_096,
                actual: 4_097
            })
        );
    }
}
