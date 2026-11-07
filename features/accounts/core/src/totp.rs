use std::fmt::{self, Debug, Formatter};

use time::OffsetDateTime;
use totp_rs::{Builder, Secret, Totp};

pub struct TotpSecret(Vec<u8>);

/// The step of time a code was accepted for, kept so the same code is refused for the rest of its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TotpStep(u64);

impl TotpSecret {
    const BYTES: usize = 20;

    pub fn drawn() -> Self {
        let mut drawn = vec![0_u8; Self::BYTES];
        getrandom::fill(&mut drawn).expect("the system draws random bytes");

        Self(drawn)
    }

    pub fn kept(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_base32(&self) -> String {
        Secret::from(self.0.clone()).to_base32()
    }

    /// The address an authenticator app takes the secret from, as its QR code holds it.
    pub fn otpauth_uri(&self, issuer: &str, label: &str) -> String {
        format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}",
            encoded(issuer),
            encoded(label),
            self.to_base32(),
            encoded(issuer)
        )
    }

    pub fn code_at(&self, at: OffsetDateTime) -> String {
        self.totp().generate(seconds(at)).to_string()
    }

    pub fn accepts(
        &self,
        code: &str,
        at: OffsetDateTime,
        last_accepted: Option<TotpStep>,
    ) -> Option<TotpStep> {
        let typed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let step = TotpStep(self.totp().check(&typed, seconds(at))?);

        last_accepted.is_none_or(|last| step > last).then_some(step)
    }

    fn totp(&self) -> Totp {
        Builder::new()
            .with_secret(self.0.clone())
            .build()
            .expect("a secret of twenty bytes makes six digits every thirty seconds")
    }
}

impl TotpStep {
    pub fn kept(step: u64) -> Self {
        Self(step)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl Debug for TotpSecret {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("TotpSecret(…)")
    }
}

fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn seconds(at: OffsetDateTime) -> u64 {
    u64::try_from(at.unix_timestamp()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;

    fn rfc_secret() -> TotpSecret {
        TotpSecret::kept(b"12345678901234567890".to_vec())
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    #[test]
    fn a_code_is_what_rfc_6238_says_it_is() {
        // given: the secret of the RFC's own test vectors
        let secret = rfc_secret();

        // expect: its codes are the RFC's, cut to six digits
        assert_eq!(secret.code_at(at(59)), "287082");
        assert_eq!(secret.code_at(at(1_111_111_109)), "081804");
        assert_eq!(secret.code_at(at(1_234_567_890)), "005924");
    }

    #[test]
    fn a_code_is_accepted_within_one_step_either_side() {
        // given: a secret and the code of one moment
        let secret = rfc_secret();
        let code = secret.code_at(at(1_111_111_109));

        // expect: it is accepted a step before and a step after
        assert!(
            secret
                .accepts(&code, at(1_111_111_109 - 30), None)
                .is_some()
        );
        assert!(
            secret
                .accepts(&code, at(1_111_111_109 + 30), None)
                .is_some()
        );
        // and: refused two steps away
        assert!(
            secret
                .accepts(&code, at(1_111_111_109 - 60), None)
                .is_none()
        );
        assert!(
            secret
                .accepts(&code, at(1_111_111_109 + 60), None)
                .is_none()
        );
    }

    #[test]
    fn a_code_once_accepted_is_refused_within_its_window() {
        // given: a code accepted once
        let secret = rfc_secret();
        let code = secret.code_at(at(1_111_111_109));
        let accepted = secret.accepts(&code, at(1_111_111_109), None);

        // expect: it is refused the second time, even within its window
        assert!(accepted.is_some());
        assert_eq!(
            secret.accepts(&code, at(1_111_111_109 + 10), accepted),
            None
        );
        // and: an older code is refused too, once a later one was accepted
        let older = secret.code_at(at(1_111_111_109 - 30));
        assert_eq!(secret.accepts(&older, at(1_111_111_109), accepted), None);
        // and: the next step's code is accepted
        let next = secret.code_at(at(1_111_111_109 + 30));
        assert!(
            secret
                .accepts(&next, at(1_111_111_109 + 30), accepted)
                .is_some()
        );
    }

    #[test]
    fn a_code_may_be_typed_with_spaces() {
        // given: a code typed in two halves
        let secret = rfc_secret();

        // expect: it is accepted as if typed in one
        assert!(secret.accepts("081 804", at(1_111_111_109), None).is_some());
    }

    #[test]
    fn anything_but_the_code_is_refused() {
        // given: a secret
        let secret = rfc_secret();

        // expect: other digits, too few, or letters are refused
        for wrong in ["081805", "08180", "0818045", "abcdef", ""] {
            assert_eq!(
                secret.accepts(wrong, at(1_111_111_109), None),
                None,
                "{wrong}"
            );
        }
    }

    #[test]
    fn a_drawn_secret_is_new_each_time_and_can_be_typed() {
        // when: two secrets are drawn
        let one = TotpSecret::drawn();
        let other = TotpSecret::drawn();

        // then: they differ
        assert_ne!(one.as_bytes(), other.as_bytes());
        // and: each can be typed as 32 letters and digits
        assert_eq!(one.to_base32().len(), 32);
        assert!(
            one.to_base32()
                .chars()
                .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
        );
    }

    #[test]
    fn an_app_takes_the_secret_with_its_issuer_and_label_from_the_uri() {
        // given: the RFC's secret
        let secret = rfc_secret();

        // expect: its address names the issuer and the account, written so no character breaks it
        assert_eq!(
            secret.otpauth_uri("Weaveling", "ada+books@example.com"),
            "otpauth://totp/Weaveling:ada%2Bbooks%40example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Weaveling"
        );
    }

    #[test]
    fn a_secret_is_not_shown_by_debugging() {
        // expect: its bytes stay out of a log
        assert_eq!(format!("{:?}", rfc_secret()), "TotpSecret(…)");
    }
}
