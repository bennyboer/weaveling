use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectName(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidProjectName {
    #[error("project name must not be blank")]
    Blank,
    #[error("project name must not contain control characters")]
    ControlCharacter,
    #[error("project name must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl ProjectName {
    pub const MAX_CHARS: usize = 200;

    pub fn new(raw: &str) -> Result<Self, InvalidProjectName> {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            return Err(InvalidProjectName::Blank);
        }

        if trimmed.chars().any(char::is_control) {
            return Err(InvalidProjectName::ControlCharacter);
        }

        let actual = trimmed.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidProjectName::TooLong {
                max: Self::MAX_CHARS,
                actual,
            });
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ProjectName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_of(chars: usize, c: char) -> String {
        std::iter::repeat_n(c, chars).collect()
    }

    #[test]
    fn accepts_a_plain_name() {
        let name = ProjectName::new("The Weaver's Apprentice").expect("should be valid");

        assert_eq!(name.as_str(), "The Weaver's Apprentice");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let name = ProjectName::new("  Tapestry \t ").expect("should be valid");

        assert_eq!(name.as_str(), "Tapestry");
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(ProjectName::new(""), Err(InvalidProjectName::Blank));
    }

    #[test]
    fn rejects_whitespace_only_input() {
        assert_eq!(ProjectName::new(" \t\n "), Err(InvalidProjectName::Blank));
    }

    #[test]
    fn rejects_inner_control_characters() {
        assert_eq!(
            ProjectName::new("two\nlines"),
            Err(InvalidProjectName::ControlCharacter)
        );
    }

    #[test]
    fn accepts_exactly_the_maximum_length() {
        let raw = name_of(ProjectName::MAX_CHARS, 'a');

        let name = ProjectName::new(&raw).expect("should be valid");

        assert_eq!(name.as_str().chars().count(), ProjectName::MAX_CHARS);
    }

    #[test]
    fn rejects_one_character_over_the_maximum() {
        let raw = name_of(ProjectName::MAX_CHARS + 1, 'a');

        assert_eq!(
            ProjectName::new(&raw),
            Err(InvalidProjectName::TooLong {
                max: ProjectName::MAX_CHARS,
                actual: ProjectName::MAX_CHARS + 1,
            })
        );
    }

    #[test]
    fn measures_length_in_characters_not_bytes() {
        let raw = name_of(ProjectName::MAX_CHARS, 'ä');

        assert!(raw.len() > ProjectName::MAX_CHARS);
        assert!(ProjectName::new(&raw).is_ok());
    }
}
