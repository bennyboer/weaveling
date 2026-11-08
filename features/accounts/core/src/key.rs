use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;
use words::{TextRules, Unfit};

ids::id!(KeyId, "key_");

pub type InvalidKeyName = Unfit;

/// What the author calls a key, to tell their keys apart: "YubiKey blue", "MacBook".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyName(String);

impl KeyName {
    pub const MAX_CHARS: usize = 60;

    const RULES: TextRules = TextRules {
        max_chars: Self::MAX_CHARS,
        may_be_blank: false,
        may_break_lines: false,
    };

    pub fn new(raw: &str) -> Result<Self, InvalidKeyName> {
        Self::RULES.fitted(raw).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for KeyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// What a key does for its account: sign in alone, as a passkey, or answer the second step after the password.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyRole {
    Primary,
    SecondFactor,
}

/// A key as its account holds it; the credential itself is a secret, kept apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    pub name: KeyName,
    pub role: KeyRole,
    pub since: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_must_be_named() {
        // expect: a blank name is refused
        assert_eq!(KeyName::new(" \t "), Err(InvalidKeyName::Blank));
    }

    #[test]
    fn a_key_name_is_one_line_up_to_its_limit() {
        // expect: a name of two lines is refused
        assert_eq!(
            KeyName::new("YubiKey\nblue"),
            Err(InvalidKeyName::ControlCharacter)
        );
        // and: one at its limit is fine
        assert!(KeyName::new(&"k".repeat(KeyName::MAX_CHARS)).is_ok());
        // and: one past its limit is refused
        assert_eq!(
            KeyName::new(&"k".repeat(KeyName::MAX_CHARS + 1)),
            Err(InvalidKeyName::TooLong {
                max: KeyName::MAX_CHARS,
                actual: KeyName::MAX_CHARS + 1,
            })
        );
    }
}
