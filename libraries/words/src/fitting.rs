use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unfit {
    Blank,
    ControlCharacter,
    TooLong { max: usize, actual: usize },
}

impl Display for Unfit {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => write!(f, "must not be blank"),
            Self::ControlCharacter => write!(f, "must not contain control characters"),
            Self::TooLong { max, actual } => {
                write!(f, "must be at most {max} characters, got {actual}")
            }
        }
    }
}

impl Error for Unfit {}

pub struct TextRules {
    pub max_chars: usize,
    pub may_be_blank: bool,
    pub may_break_lines: bool,
}

impl TextRules {
    pub fn fitted(&self, raw: &str) -> Result<String, Unfit> {
        let trimmed = raw.trim();

        if trimmed.is_empty() && !self.may_be_blank {
            return Err(Unfit::Blank);
        }
        if trimmed
            .chars()
            .any(|sign| sign.is_control() && !(self.may_break_lines && sign == '\n'))
        {
            return Err(Unfit::ControlCharacter);
        }
        let actual = trimmed.chars().count();
        if actual > self.max_chars {
            return Err(Unfit::TooLong {
                max: self.max_chars,
                actual,
            });
        }

        Ok(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITLE: TextRules = TextRules {
        max_chars: 10,
        may_be_blank: true,
        may_break_lines: false,
    };

    const NAME: TextRules = TextRules {
        max_chars: 10,
        may_be_blank: false,
        may_break_lines: false,
    };

    const DESCRIPTION: TextRules = TextRules {
        max_chars: 10,
        may_be_blank: true,
        may_break_lines: true,
    };

    #[test]
    fn what_is_written_is_kept_without_its_margins() {
        // expect: text is kept without its margins
        assert_eq!(TITLE.fitted("  The Loom  "), Ok("The Loom".to_owned()));
    }

    #[test]
    fn nothing_but_whitespace_is_blank() {
        // expect: whitespace alone is blank where blank is allowed
        assert_eq!(TITLE.fitted(" \t "), Ok(String::new()));
        // and: refused where it is not
        assert_eq!(NAME.fitted(" \t "), Err(Unfit::Blank));
    }

    #[test]
    fn control_characters_are_refused_and_line_breaks_only_where_allowed() {
        // expect: a line break is refused where none is allowed
        assert_eq!(TITLE.fitted("The\nLoom"), Err(Unfit::ControlCharacter));
        // and: kept where it is
        assert_eq!(DESCRIPTION.fitted("The\nLoom"), Ok("The\nLoom".to_owned()));
        // and: another control character is refused even there
        assert_eq!(DESCRIPTION.fitted("a\u{7}b"), Err(Unfit::ControlCharacter));
    }

    #[test]
    fn length_is_counted_in_characters_up_to_the_limit() {
        // expect: as many characters as the limit fit, however many bytes they take
        assert!(TITLE.fitted(&"é".repeat(10)).is_ok());
        // and: one more is too long
        assert_eq!(
            TITLE.fitted(&"a".repeat(11)),
            Err(Unfit::TooLong {
                max: 10,
                actual: 11
            })
        );
    }

    #[test]
    fn what_is_wrong_is_said_with_its_numbers() {
        // expect: too long says by how much
        assert_eq!(
            Unfit::TooLong {
                max: 10,
                actual: 11
            }
            .to_string(),
            "must be at most 10 characters, got 11"
        );
    }
}
