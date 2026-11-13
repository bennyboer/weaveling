use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime};

use crate::session::hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodePurpose {
    ConfirmAddress,
    ResetPassword,
}

impl CodePurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConfirmAddress => "confirm-address",
            Self::ResetPassword => "reset-password",
        }
    }
}

/// A code as it is mailed.
/// It is stored nowhere except in the mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCode(String);

/// What is stored of a code: its SHA-256 hash.
/// So reading the database or a backup reveals no code that could be typed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeDigest(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingVerification {
    pub digest: CodeDigest,
    pub until: OffsetDateTime,
    pub tried: u32,
}

impl VerificationCode {
    pub const LASTS: Duration = Duration::minutes(15);

    const POSSIBLE_CODES: u32 = 1_000_000;
    const FAIR_BELOW: u32 = u32::MAX - u32::MAX % Self::POSSIBLE_CODES;

    pub fn drawn() -> Self {
        Self::fairly_from(|| getrandom::u32().expect("the system draws random bytes"))
    }

    fn fairly_from(mut draw: impl FnMut() -> u32) -> Self {
        loop {
            let drawn = draw();
            if drawn < Self::FAIR_BELOW {
                return Self(format!("{:06}", drawn % Self::POSSIBLE_CODES));
            }
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> CodeDigest {
        CodeDigest::of(&self.0)
    }
}

impl CodeDigest {
    fn of(code: &str) -> Self {
        Self(hex(&Sha256::digest(code.as_bytes())))
    }

    pub fn kept(digest: String) -> Self {
        Self(digest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether a typed code is the one this digest was made of, ignoring spaces around it.
    /// The check takes the same time, no matter how much of the code is right.
    pub fn admits(&self, tried: &str) -> bool {
        Self::of(tried.trim())
            .0
            .as_bytes()
            .ct_eq(self.0.as_bytes())
            .into()
    }
}

impl PendingVerification {
    pub const MAX_TRIES: u32 = 5;

    /// Creates a new code to be mailed.
    /// Also returns what is stored of it, and until when it is valid.
    pub fn issued(now: OffsetDateTime) -> (VerificationCode, Self) {
        let code = VerificationCode::drawn();
        let pending = Self {
            digest: code.digest(),
            until: now + VerificationCode::LASTS,
            tried: 0,
        };

        (code, pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_six_digits_to_copy_from_a_mail() {
        // expect: every code drawn is six digits
        for _ in 0..100 {
            let code = VerificationCode::drawn();

            assert_eq!(code.as_str().len(), 6, "{code:?}");
            assert!(code.as_str().bytes().all(|digit| digit.is_ascii_digit()));
        }
    }

    #[test]
    fn a_code_is_drawn_anew_each_time() {
        // when: codes are drawn again and again
        let drawn: Vec<_> = (0..20).map(|_| VerificationCode::drawn()).collect();

        // then: they are not all the same
        assert!(drawn.iter().any(|code| *code != drawn[0]));
    }

    #[test]
    fn a_draw_beyond_the_last_whole_million_is_thrown_back() {
        // given: draws beyond the last whole million before one within it
        let mut draws = [u32::MAX, 4_294_000_000, 4_293_999_999].into_iter();

        // expect: the code is made of the first draw within it
        assert_eq!(
            VerificationCode::fairly_from(|| draws.next().expect("a draw left")).as_str(),
            "999999",
            "the draws past 4,293,999,999 would make the lowest codes a little likelier, \
             so they are drawn again rather than folded in"
        );
    }

    #[test]
    fn a_small_draw_is_padded_to_six_digits() {
        // expect: a draw of few digits makes a code led by zeros
        assert_eq!(VerificationCode::fairly_from(|| 42).as_str(), "000042");
    }

    #[test]
    fn a_code_pasted_with_spaces_around_it_still_counts() {
        // given: what is kept of a code led by zeros
        let kept = CodeDigest::of("004217");

        // then: it admits the code pasted with spaces around it
        assert!(kept.admits(" 004217\n"));
        // and: not the code without its zeros
        assert!(!kept.admits("4217"));
    }

    #[test]
    fn an_issued_code_lasts_a_quarter_of_an_hour_and_is_untried() {
        // when: a code is issued
        let now = OffsetDateTime::UNIX_EPOCH;
        let (_, pending) = PendingVerification::issued(now);

        // then: it lasts fifteen minutes and has not been tried
        assert_eq!(pending.until, now + Duration::minutes(15));
        assert_eq!(pending.tried, 0);
    }
}

#[cfg(test)]
mod digest_tests {
    use super::*;

    #[test]
    fn what_is_kept_of_a_code_is_its_sha_256_and_admits_it_alone() {
        // given: a code issued
        let (code, pending) = PendingVerification::issued(OffsetDateTime::UNIX_EPOCH);

        // expect: what is kept is no code, but 64 hex digits
        assert_ne!(pending.digest.as_str(), code.as_str());
        assert_eq!(pending.digest.as_str().len(), 64);
        // and: it admits the code, around spaces
        assert!(pending.digest.admits(&format!(" {} ", code.as_str())));
        // and: no other
        let other = if code.as_str() == "000000" {
            "000001"
        } else {
            "000000"
        };
        assert!(!pending.digest.admits(other));
        // and: the digest of 000000 is its sha 256
        assert_eq!(
            CodeDigest::of("000000").as_str(),
            "91b4d142823f7d20c5f08df69122de43f35f057a988d9619f6d3138485c9a203"
        );
    }
}
