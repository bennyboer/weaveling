use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct PieceTitle(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidPieceTitle {
    #[error("a piece title must not contain control characters")]
    ControlCharacter,
    #[error("a piece title must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl PieceTitle {
    pub const MAX_CHARS: usize = 2048;

    pub fn new(raw: &str) -> Result<Self, InvalidPieceTitle> {
        let trimmed = raw.trim();

        if trimmed.chars().any(char::is_control) {
            return Err(InvalidPieceTitle::ControlCharacter);
        }

        let actual = trimmed.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidPieceTitle::TooLong {
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

impl Display for PieceTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_keeps_what_the_author_wrote() {
        let title = PieceTitle::new("The Loom").expect("a plain title is fine");

        assert_eq!(title.as_str(), "The Loom");
        assert_eq!(title.to_string(), "The Loom");
    }

    #[test]
    fn an_empty_title_is_allowed_because_an_idea_arrives_before_its_name() {
        let title = PieceTitle::new("").expect("empty is a legal title");

        assert!(title.is_untitled());
        assert_eq!(title.as_str(), "");
    }

    #[test]
    fn whitespace_only_is_the_same_as_untitled() {
        let title = PieceTitle::new("   \t  ").expect("whitespace trims to empty");

        assert!(
            title.is_untitled(),
            "there must be exactly one way to be untitled"
        );
        assert_eq!(title, PieceTitle::untitled());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_away() {
        let title = PieceTitle::new("  The Loom  ").expect("a plain title is fine");

        assert_eq!(title.as_str(), "The Loom");
    }

    #[test]
    fn a_titled_piece_is_not_untitled() {
        assert!(!PieceTitle::new("The Loom").expect("fine").is_untitled());
    }

    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            PieceTitle::new("The\nLoom"),
            Err(InvalidPieceTitle::ControlCharacter)
        );
    }

    #[test]
    fn a_title_longer_than_the_limit_is_refused() {
        let sprawling = "a".repeat(PieceTitle::MAX_CHARS + 1);

        assert_eq!(
            PieceTitle::new(&sprawling),
            Err(InvalidPieceTitle::TooLong {
                max: PieceTitle::MAX_CHARS,
                actual: PieceTitle::MAX_CHARS + 1,
            })
        );
    }

    #[test]
    fn a_title_at_the_limit_is_accepted() {
        let long = "a".repeat(PieceTitle::MAX_CHARS);

        assert!(PieceTitle::new(&long).is_ok());
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        let accented = "é".repeat(PieceTitle::MAX_CHARS);

        assert!(
            PieceTitle::new(&accented).is_ok(),
            "a two-byte character is still one character to an author"
        );
    }
}
