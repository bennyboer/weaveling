use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PassageTitle(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidPassageTitle {
    #[error("a passage title must not contain control characters")]
    ControlCharacter,
    #[error("a passage title must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl PassageTitle {
    pub const MAX_CHARS: usize = 2048;

    pub fn new(raw: &str) -> Result<Self, InvalidPassageTitle> {
        let trimmed = raw.trim();

        if trimmed.chars().any(char::is_control) {
            return Err(InvalidPassageTitle::ControlCharacter);
        }

        let actual = trimmed.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidPassageTitle::TooLong {
                max: Self::MAX_CHARS,
                actual,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn untitled() -> Self {
        Self(String::new())
    }

    pub fn is_untitled(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PassageTitle {
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
            PassageTitle::new("  The loom  ").unwrap().as_str(),
            "The loom"
        );
    }

    #[test]
    fn a_passage_may_go_untitled_so_the_outline_falls_back_to_its_opening_words() {
        assert!(PassageTitle::new("   ").unwrap().is_untitled());
        assert!(PassageTitle::untitled().is_untitled());
    }

    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            PassageTitle::new("The\u{7}loom"),
            Err(InvalidPassageTitle::ControlCharacter)
        );
    }

    #[test]
    fn a_title_longer_than_the_cap_is_refused() {
        let far_too_long = "a".repeat(PassageTitle::MAX_CHARS + 1);

        assert_eq!(
            PassageTitle::new(&far_too_long),
            Err(InvalidPassageTitle::TooLong {
                max: PassageTitle::MAX_CHARS,
                actual: PassageTitle::MAX_CHARS + 1,
            })
        );
    }
}
