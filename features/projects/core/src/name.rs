use std::fmt::{self, Display, Formatter};

use words::{TextRules, Unfit};

pub type InvalidProjectName = Unfit;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectName(String);

impl ProjectName {
    pub const MAX_CHARS: usize = 200;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: false,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidProjectName> {
        Self::RULES.fitted(raw).map(Self)
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

    #[test]
    fn a_project_must_be_named() {
        assert_eq!(ProjectName::new("  "), Err(InvalidProjectName::Blank));
    }

    #[test]
    fn a_name_is_one_line_up_to_its_limit() {
        assert_eq!(
            ProjectName::new("The\nLoom"),
            Err(InvalidProjectName::ControlCharacter)
        );
        assert!(ProjectName::new(&"a".repeat(ProjectName::MAX_CHARS)).is_ok());
        assert_eq!(
            ProjectName::new(&"a".repeat(ProjectName::MAX_CHARS + 1)),
            Err(InvalidProjectName::TooLong {
                max: ProjectName::MAX_CHARS,
                actual: ProjectName::MAX_CHARS + 1,
            })
        );
    }
}
