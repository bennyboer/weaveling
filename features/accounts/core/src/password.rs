use argon2::password_hash::{PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, PasswordHash as Phc};
use rand_core::OsRng;
use thiserror::Error;

pub struct Password(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidPassword {
    #[error("must be at least {min} characters, got {actual}")]
    TooShort { min: usize, actual: usize },
    #[error("must be at most {max} characters, got {actual}")]
    TooLong { max: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl Password {
    pub const MIN_CHARS: usize = 12;
    pub const MAX_CHARS: usize = 256;

    pub fn new(raw: &str) -> Result<Self, InvalidPassword> {
        let actual = raw.chars().count();
        if actual < Self::MIN_CHARS {
            return Err(InvalidPassword::TooShort {
                min: Self::MIN_CHARS,
                actual,
            });
        }
        if actual > Self::MAX_CHARS {
            return Err(InvalidPassword::TooLong {
                max: Self::MAX_CHARS,
                actual,
            });
        }

        Ok(Self(raw.to_owned()))
    }

    pub fn hashed(&self) -> PasswordHash {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(self.0.as_bytes(), &salt)
            .expect("Argon2 with its default parameters hashes any password");

        PasswordHash(hash.to_string())
    }
}

impl PasswordHash {
    pub fn kept(phc: String) -> Self {
        Self(phc)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn admits(&self, tried: &str) -> bool {
        Phc::new(&self.0).is_ok_and(|kept| {
            Argon2::default()
                .verify_password(tried.as_bytes(), &kept)
                .is_ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A_PASSWORD: &str = "correct horse battery";

    #[test]
    fn a_password_is_long_enough_to_resist_guessing_and_bounded() {
        assert!(Password::new(&"a".repeat(Password::MIN_CHARS)).is_ok());
        assert!(Password::new(&"a".repeat(Password::MAX_CHARS)).is_ok());
        assert_eq!(
            Password::new(&"a".repeat(Password::MIN_CHARS - 1)).err(),
            Some(InvalidPassword::TooShort {
                min: Password::MIN_CHARS,
                actual: Password::MIN_CHARS - 1,
            })
        );
        assert_eq!(
            Password::new(&"a".repeat(Password::MAX_CHARS + 1)).err(),
            Some(InvalidPassword::TooLong {
                max: Password::MAX_CHARS,
                actual: Password::MAX_CHARS + 1,
            })
        );
    }

    #[test]
    fn a_password_is_kept_as_it_was_typed_spaces_and_all() {
        let hash = Password::new("  spaced out  ")
            .expect("long enough")
            .hashed();

        assert!(hash.admits("  spaced out  "));
        assert!(!hash.admits("spaced out"));
    }

    #[test]
    fn a_hash_admits_its_own_password_and_no_other() {
        let hash = Password::new(A_PASSWORD).expect("long enough").hashed();

        assert!(hash.admits(A_PASSWORD));
        assert!(!hash.admits("incorrect horse battery"));
    }

    #[test]
    fn a_hash_is_argon2id_and_salted_anew_each_time() {
        let password = Password::new(A_PASSWORD).expect("long enough");
        let first = password.hashed();

        assert!(
            first.as_str().starts_with("$argon2id$"),
            "{}",
            first.as_str()
        );
        assert_ne!(
            first,
            password.hashed(),
            "the same password hashed twice must not look the same in the table"
        );
        assert!(!first.as_str().contains(A_PASSWORD));
    }

    #[test]
    fn a_kept_hash_that_cannot_be_read_admits_nothing() {
        assert!(!PasswordHash::kept("scribbled by hand".to_owned()).admits(A_PASSWORD));
    }
}
