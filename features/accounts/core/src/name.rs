use std::fmt::{self, Display, Formatter};

use words::{TextRules, Unfit};

pub type InvalidAccountName = Unfit;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccountName(String);

impl AccountName {
    pub const MAX_CHARS: usize = 100;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: false,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidAccountName> {
        Self::RULES.fitted(raw).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for AccountName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_must_be_named() {
        assert_eq!(AccountName::new("  "), Err(InvalidAccountName::Blank));
    }

    #[test]
    fn a_name_is_one_line_up_to_its_limit() {
        assert_eq!(
            AccountName::new("Ada\nLovelace"),
            Err(InvalidAccountName::ControlCharacter)
        );
        assert!(AccountName::new(&"a".repeat(AccountName::MAX_CHARS)).is_ok());
        assert_eq!(
            AccountName::new(&"a".repeat(AccountName::MAX_CHARS + 1)),
            Err(InvalidAccountName::TooLong {
                max: AccountName::MAX_CHARS,
                actual: AccountName::MAX_CHARS + 1,
            })
        );
    }
}
