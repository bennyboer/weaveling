use std::fmt::{self, Debug, Display, Formatter};

use argon2::password_hash::{PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Argon2, PasswordHash as Phc};
use rand_core::{OsRng, RngCore};

pub struct RecoveryCode(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCodeHash(String);

impl RecoveryCode {
    pub const DRAWN: usize = 10;
    const CHARS: usize = 10;
    const ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";

    pub fn drawn() -> Vec<Self> {
        (0..Self::DRAWN).map(|_| Self::one_drawn()).collect()
    }

    pub fn presented(raw: &str) -> Option<Self> {
        let typed: String = raw
            .chars()
            .filter(|c| *c != '-' && !c.is_whitespace())
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let fits = typed.len() == Self::CHARS && typed.bytes().all(|b| Self::ALPHABET.contains(&b));

        fits.then_some(Self(typed))
    }

    pub fn hashed(&self) -> RecoveryCodeHash {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(self.0.as_bytes(), &salt)
            .expect("Argon2 with its default parameters hashes any code");

        RecoveryCodeHash(hash.to_string())
    }

    fn one_drawn() -> Self {
        let fair = u8::MAX - u8::MAX % Self::ALPHABET.len() as u8;
        let mut drawn = String::with_capacity(Self::CHARS);
        while drawn.len() < Self::CHARS {
            let byte = (OsRng.next_u32() & 0xff) as u8;
            if byte < fair {
                drawn.push(Self::ALPHABET[usize::from(byte) % Self::ALPHABET.len()] as char);
            }
        }

        Self(drawn)
    }
}

impl RecoveryCodeHash {
    pub fn kept(phc: String) -> Self {
        Self(phc)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn admits(&self, code: &RecoveryCode) -> bool {
        Phc::new(&self.0).is_ok_and(|kept| {
            Argon2::default()
                .verify_password(code.0.as_bytes(), &kept)
                .is_ok()
        })
    }
}

impl Display for RecoveryCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (front, back) = self.0.split_at(Self::CHARS / 2);
        write!(f, "{front}-{back}")
    }
}

impl Debug for RecoveryCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(…)")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ten_codes_are_drawn_of_ten_characters_none_easily_confused() {
        // when: codes are drawn
        let drawn = RecoveryCode::drawn();

        // then: there are ten, each different
        assert_eq!(drawn.len(), 10);
        assert_eq!(
            drawn
                .iter()
                .map(ToString::to_string)
                .collect::<HashSet<_>>()
                .len(),
            10
        );
        // and: each shows as two halves of five, without 0, 1, i, l or o
        for code in &drawn {
            let shown = code.to_string();
            assert_eq!(shown.len(), 11, "{shown}");
            assert_eq!(&shown[5..6], "-", "{shown}");
            assert!(!shown.contains(['0', '1', 'i', 'l', 'o']), "{shown}");
        }
    }

    #[test]
    fn a_code_is_read_alike_with_or_without_the_dash_and_in_either_case() {
        // expect: every way of typing it reads as the same code
        for typed in ["k7qp3-mx9tw", "k7qp3mx9tw", "K7QP3-MX9TW", " k7qp3 mx9tw "] {
            assert_eq!(
                RecoveryCode::presented(typed).map(|code| code.to_string()),
                Some("k7qp3-mx9tw".to_owned()),
                "{typed}"
            );
        }
    }

    #[test]
    fn a_code_of_another_shape_is_no_code() {
        // expect: too short, too long or with a character never drawn is refused
        for typed in [
            "k7qp3-mx9t",
            "k7qp3-mx9twa",
            "k7qp0-mx9tw",
            "k7qp3_mx9tw",
            "",
        ] {
            assert!(RecoveryCode::presented(typed).is_none(), "{typed}");
        }
    }

    #[test]
    fn a_hashed_code_admits_that_code_alone() {
        // given: a code kept hashed
        let code = RecoveryCode::presented("k7qp3-mx9tw").expect("a code");
        let hash = code.hashed();

        // expect: it admits the code, however typed
        assert!(hash.admits(&RecoveryCode::presented("K7QP3MX9TW").expect("a code")));
        // and: no other
        assert!(!hash.admits(&RecoveryCode::presented("k7qp3-mx9tx").expect("a code")));
        // and: the hash holds not the code itself
        assert!(!hash.as_str().contains("k7qp3mx9tw"));
    }

    #[test]
    fn a_code_is_not_shown_by_debugging() {
        // expect: it stays out of a log
        let code = RecoveryCode::presented("k7qp3-mx9tw").expect("a code");
        assert_eq!(format!("{code:?}"), "RecoveryCode(…)");
    }
}
