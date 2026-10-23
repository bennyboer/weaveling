use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmailAddress(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidEmailAddress {
    #[error("must be an address like name@example.com")]
    Malformed,
    #[error("must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

impl EmailAddress {
    pub const MAX_CHARS: usize = 254;

    pub fn new(raw: &str) -> Result<Self, InvalidEmailAddress> {
        let address = raw.trim().to_lowercase();
        let actual = address.chars().count();
        if actual > Self::MAX_CHARS {
            return Err(InvalidEmailAddress::TooLong {
                max: Self::MAX_CHARS,
                actual,
            });
        }

        let Some((local, domain)) = address.split_once('@') else {
            return Err(InvalidEmailAddress::Malformed);
        };
        let well_formed = !local.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && !domain.contains('@')
            && !address
                .chars()
                .any(|sign| sign.is_whitespace() || sign.is_control());
        if !well_formed {
            return Err(InvalidEmailAddress::Malformed);
        }

        Ok(Self(address))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for EmailAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_kept_trimmed_and_in_lower_case() {
        assert_eq!(
            EmailAddress::new("  Ada@Example.COM ").map(|address| address.to_string()),
            Ok("ada@example.com".to_owned()),
            "one person, however they type their address, is one account"
        );
    }

    #[test]
    fn an_address_may_carry_a_tag_after_a_plus() {
        assert_eq!(
            EmailAddress::new("test+test@gmail.com").map(|address| address.to_string()),
            Ok("test+test@gmail.com".to_owned()),
            "a tagged address is the person's own choice, kept apart from the untagged one"
        );
    }

    #[test]
    fn an_address_needs_someone_at_a_domain_with_a_dot() {
        for malformed in [
            "ada",
            "@example.com",
            "ada@",
            "ada@example",
            "ada@.com",
            "ada@example.",
            "ada@ex@ample.com",
            "a da@example.com",
            "ada@exa\u{7}mple.com",
        ] {
            assert_eq!(
                EmailAddress::new(malformed),
                Err(InvalidEmailAddress::Malformed),
                "{malformed}"
            );
        }
    }

    #[test]
    fn an_address_is_at_most_as_long_as_mail_allows() {
        let longest = format!("{}@example.com", "a".repeat(EmailAddress::MAX_CHARS - 12));

        assert!(EmailAddress::new(&longest).is_ok());
        assert_eq!(
            EmailAddress::new(&format!("a{longest}")),
            Err(InvalidEmailAddress::TooLong {
                max: EmailAddress::MAX_CHARS,
                actual: EmailAddress::MAX_CHARS + 1,
            })
        );
    }
}
