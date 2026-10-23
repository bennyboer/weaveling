use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;
use words::{TextRules, Unfit};

pub type InvalidLockReason = Unfit;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockReason(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockKind {
    Operator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    pub reason: LockReason,
    pub since: OffsetDateTime,
}

impl LockReason {
    pub const MAX_CHARS: usize = 500;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: false,
        may_break_lines: true,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidLockReason> {
        Self::RULES.fitted(raw).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for LockReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_is_locked_for_a_reason() {
        assert_eq!(LockReason::new("  "), Err(InvalidLockReason::Blank));
    }

    #[test]
    fn a_reason_may_take_a_few_lines_up_to_its_limit() {
        assert!(LockReason::new("Sent spam.\nAsked to be locked out for now.").is_ok());
        assert!(LockReason::new(&"a".repeat(LockReason::MAX_CHARS)).is_ok());
        assert_eq!(
            LockReason::new(&"a".repeat(LockReason::MAX_CHARS + 1)),
            Err(InvalidLockReason::TooLong {
                max: LockReason::MAX_CHARS,
                actual: LockReason::MAX_CHARS + 1,
            })
        );
    }
}
