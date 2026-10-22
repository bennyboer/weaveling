use rand_core::{OsRng, RngCore};
use time::{Duration, OffsetDateTime};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCode(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingVerification {
    pub code: VerificationCode,
    pub until: OffsetDateTime,
    pub tried: u32,
}

impl VerificationCode {
    pub const LASTS: Duration = Duration::minutes(15);

    const POSSIBLE_CODES: u32 = 1_000_000;
    const FAIR_BELOW: u32 = u32::MAX - u32::MAX % Self::POSSIBLE_CODES;

    pub fn drawn() -> Self {
        Self::fairly_from(|| OsRng.next_u32())
    }

    fn fairly_from(mut draw: impl FnMut() -> u32) -> Self {
        loop {
            let drawn = draw();
            if drawn < Self::FAIR_BELOW {
                return Self(format!("{:06}", drawn % Self::POSSIBLE_CODES));
            }
        }
    }

    pub fn kept(code: String) -> Self {
        Self(code)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is(&self, tried: &str) -> bool {
        tried.trim() == self.0
    }
}

impl PendingVerification {
    pub const MAX_TRIES: u32 = 5;

    pub fn issued(now: OffsetDateTime) -> Self {
        Self {
            code: VerificationCode::drawn(),
            until: now + VerificationCode::LASTS,
            tried: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_six_digits_to_copy_from_a_mail() {
        for _ in 0..100 {
            let code = VerificationCode::drawn();

            assert_eq!(code.as_str().len(), 6, "{code:?}");
            assert!(code.as_str().bytes().all(|digit| digit.is_ascii_digit()));
        }
    }

    #[test]
    fn a_code_is_drawn_anew_each_time() {
        let drawn: Vec<_> = (0..20).map(|_| VerificationCode::drawn()).collect();

        assert!(drawn.iter().any(|code| *code != drawn[0]));
    }

    #[test]
    fn a_draw_beyond_the_last_whole_million_is_thrown_back() {
        let mut draws = [u32::MAX, 4_294_000_000, 4_293_999_999].into_iter();

        assert_eq!(
            VerificationCode::fairly_from(|| draws.next().expect("a draw left")),
            VerificationCode::kept("999999".to_owned()),
            "the draws past 4,293,999,999 would make the lowest codes a little likelier, \
             so they are drawn again rather than folded in"
        );
    }

    #[test]
    fn a_small_draw_is_padded_to_six_digits() {
        assert_eq!(
            VerificationCode::fairly_from(|| 42),
            VerificationCode::kept("000042".to_owned())
        );
    }

    #[test]
    fn a_code_pasted_with_spaces_around_it_still_counts() {
        let code = VerificationCode::kept("004217".to_owned());

        assert!(code.is(" 004217\n"));
        assert!(!code.is("4217"));
    }

    #[test]
    fn an_issued_code_lasts_a_quarter_of_an_hour_and_is_untried() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let pending = PendingVerification::issued(now);

        assert_eq!(pending.until, now + Duration::minutes(15));
        assert_eq!(pending.tried, 0);
    }
}
