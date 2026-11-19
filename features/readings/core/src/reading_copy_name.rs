use std::fmt::{self, Display, Formatter};

use words::{TextRules, Unfit};

pub type InvalidReadingCopyName = Unfit;

/// What a reading copy is called: one line, never blank.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReadingCopyName(String);

impl ReadingCopyName {
    pub const MAX_CHARS: usize = 200;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: false,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidReadingCopyName> {
        Self::RULES.fitted(raw).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ReadingCopyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_one_line_and_never_blank() {
        // expect: a name of only blanks, one breaking its line, and one past the limit are refused
        assert!(ReadingCopyName::new("  ").is_err());
        assert!(ReadingCopyName::new("For\nLena").is_err());
        assert!(ReadingCopyName::new(&"a".repeat(ReadingCopyName::MAX_CHARS + 1)).is_err());
        // and: a plain one is kept as it is written
        assert_eq!(
            ReadingCopyName::new("For Lena")
                .expect("a plain name is fine")
                .as_str(),
            "For Lena"
        );
    }
}
