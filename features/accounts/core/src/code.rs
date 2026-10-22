use rand_core::{OsRng, RngCore};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCode(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingVerification {
    pub code: VerificationCode,
    pub until: OffsetDateTime,
    pub failed: u32,
}

impl VerificationCode {
    pub const LASTS: Duration = Duration::minutes(15);

    pub fn drawn() -> Self {
        Self(format!("{:06}", OsRng.next_u32() % 1_000_000))
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
    pub const MAX_FAILED: u32 = 5;

    pub fn issued(now: OffsetDateTime) -> Self {
        Self {
            code: VerificationCode::drawn(),
            until: now + VerificationCode::LASTS,
            failed: 0,
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
    fn a_code_pasted_with_spaces_around_it_still_counts() {
        let code = VerificationCode::kept("004217".to_owned());

        assert!(code.is(" 004217\n"));
        assert!(!code.is("4217"));
    }

    #[test]
    fn an_issued_code_lasts_a_quarter_of_an_hour_and_has_failed_nothing() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let pending = PendingVerification::issued(now);

        assert_eq!(pending.until, now + Duration::minutes(15));
        assert_eq!(pending.failed, 0);
    }
}
