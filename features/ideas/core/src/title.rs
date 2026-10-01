use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct IdeaTitle(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidIdeaTitle {
    #[error("a idea title must not contain control characters")]
    ControlCharacter,
    #[error("a idea title must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl IdeaTitle {
    pub const MAX_CHARS: usize = 2048;

    pub fn new(raw: &str) -> Result<Self, InvalidIdeaTitle> {
        let trimmed = raw.trim();

        if trimmed.chars().any(char::is_control) {
            return Err(InvalidIdeaTitle::ControlCharacter);
        }

        let actual = trimmed.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidIdeaTitle::TooLong {
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

impl Display for IdeaTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_keeps_what_the_author_wrote() {
        let title = IdeaTitle::new("The Loom").expect("a plain title is fine");

        assert_eq!(title.as_str(), "The Loom");
        assert_eq!(title.to_string(), "The Loom");
    }

    #[test]
    fn an_empty_title_is_allowed_because_an_idea_arrives_before_its_name() {
        let title = IdeaTitle::new("").expect("empty is a legal title");

        assert!(title.is_untitled());
        assert_eq!(title.as_str(), "");
    }

    #[test]
    fn whitespace_only_is_the_same_as_untitled() {
        let title = IdeaTitle::new("   \t  ").expect("whitespace trims to empty");

        assert!(
            title.is_untitled(),
            "there must be exactly one way to be untitled"
        );
        assert_eq!(title, IdeaTitle::untitled());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed_away() {
        let title = IdeaTitle::new("  The Loom  ").expect("a plain title is fine");

        assert_eq!(title.as_str(), "The Loom");
    }

    #[test]
    fn a_titled_idea_is_not_untitled() {
        assert!(!IdeaTitle::new("The Loom").expect("fine").is_untitled());
    }

    #[test]
    fn control_characters_are_refused() {
        assert_eq!(
            IdeaTitle::new("The\nLoom"),
            Err(InvalidIdeaTitle::ControlCharacter)
        );
    }

    #[test]
    fn a_title_longer_than_the_limit_is_refused() {
        let sprawling = "a".repeat(IdeaTitle::MAX_CHARS + 1);

        assert_eq!(
            IdeaTitle::new(&sprawling),
            Err(InvalidIdeaTitle::TooLong {
                max: IdeaTitle::MAX_CHARS,
                actual: IdeaTitle::MAX_CHARS + 1,
            })
        );
    }

    #[test]
    fn a_title_at_the_limit_is_accepted() {
        let long = "a".repeat(IdeaTitle::MAX_CHARS);

        assert!(IdeaTitle::new(&long).is_ok());
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        let accented = "é".repeat(IdeaTitle::MAX_CHARS);

        assert!(
            IdeaTitle::new(&accented).is_ok(),
            "a two-byte character is still one character to an author"
        );
    }
}
