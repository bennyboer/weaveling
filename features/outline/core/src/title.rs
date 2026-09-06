use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SectionTitle(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidSectionTitle {
    #[error("a section title must not contain control characters")]
    ControlCharacter,
    #[error("a section title must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl SectionTitle {
    pub const MAX_CHARS: usize = 2048;

    pub fn new(raw: &str) -> Result<Self, InvalidSectionTitle> {
        let trimmed = raw.trim();

        if trimmed.chars().any(char::is_control) {
            return Err(InvalidSectionTitle::ControlCharacter);
        }

        let actual = trimmed.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidSectionTitle::TooLong {
                max: Self::MAX_CHARS,
                actual,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn unnamed() -> Self {
        Self(String::new())
    }

    pub fn is_unnamed(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for SectionTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_trimmed() {
        assert_eq!(
            SectionTitle::new("  Chapter 1  ").unwrap().as_str(),
            "Chapter 1"
        );
    }

    #[test]
    fn a_section_may_go_unnamed_so_it_can_borrow_its_piece_s_title() {
        assert!(SectionTitle::new("   ").unwrap().is_unnamed());
        assert!(SectionTitle::unnamed().is_unnamed());
    }

    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            SectionTitle::new("Chapter\u{7}1"),
            Err(InvalidSectionTitle::ControlCharacter)
        );
    }

    #[test]
    fn a_title_longer_than_the_cap_is_refused() {
        let far_too_long = "a".repeat(SectionTitle::MAX_CHARS + 1);

        assert_eq!(
            SectionTitle::new(&far_too_long),
            Err(InvalidSectionTitle::TooLong {
                max: SectionTitle::MAX_CHARS,
                actual: SectionTitle::MAX_CHARS + 1,
            })
        );
    }
}
