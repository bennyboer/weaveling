use std::fmt::{self, Display, Formatter};

use words::{TextRules, Unfit};

pub type InvalidSectionTitle = Unfit;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SectionTitle(String);

impl SectionTitle {
    pub const MAX_CHARS: usize = 2048;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: true,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidSectionTitle> {
        Self::RULES.fitted(raw).map(Self)
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
    fn a_section_may_be_unnamed() {
        // when: a title of nothing but blanks is made
        let title = SectionTitle::new("  ").expect("a section may be unnamed");

        // then: it is the unnamed title
        assert_eq!(title, SectionTitle::unnamed());
        assert!(title.is_unnamed());
    }

    #[test]
    fn a_title_is_one_line_up_to_its_limit() {
        // expect: a title spanning lines is refused
        assert_eq!(
            SectionTitle::new("The\nLoom"),
            Err(InvalidSectionTitle::ControlCharacter)
        );
        // and: one at its limit is made
        assert!(SectionTitle::new(&"a".repeat(SectionTitle::MAX_CHARS)).is_ok());
        // and: one past its limit is refused
        assert_eq!(
            SectionTitle::new(&"a".repeat(SectionTitle::MAX_CHARS + 1)),
            Err(InvalidSectionTitle::TooLong {
                max: SectionTitle::MAX_CHARS,
                actual: SectionTitle::MAX_CHARS + 1,
            })
        );
    }
}
