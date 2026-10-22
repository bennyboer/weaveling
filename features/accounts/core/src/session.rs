use std::fmt::{self, Debug, Formatter};

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

use crate::id::AccountId;

pub struct SessionToken(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenDigest(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    SecondFactorOwed,
    SignedIn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub account: AccountId,
    pub stage: Stage,
    pub begun: OffsetDateTime,
    pub until: OffsetDateTime,
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl SessionToken {
    const BYTES: usize = 32;

    pub fn drawn() -> Self {
        let mut drawn = [0_u8; Self::BYTES];
        OsRng.fill_bytes(&mut drawn);

        Self(hex(&drawn))
    }

    pub fn presented(raw: &str) -> Self {
        Self(raw.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> TokenDigest {
        TokenDigest::of(&self.0)
    }
}

impl Debug for SessionToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(…)")
    }
}

impl TokenDigest {
    pub(crate) fn of(raw: &str) -> Self {
        Self(hex(&Sha256::digest(raw.as_bytes())))
    }

    pub fn kept(digest: String) -> Self {
        Self(digest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Session {
    pub const LASTS: Duration = Duration::days(30);

    pub fn begun(account: AccountId, stage: Stage, now: OffsetDateTime) -> Self {
        Self {
            account,
            stage,
            begun: now,
            until: now + Self::LASTS,
        }
    }

    pub fn is_over(&self, now: OffsetDateTime) -> bool {
        now > self.until
    }

    pub fn may_act(&self) -> bool {
        self.stage == Stage::SignedIn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn an_account() -> AccountId {
        AccountId::generate(at(1_000))
    }

    #[test]
    fn a_token_is_256_random_bits_written_in_hex() {
        let token = SessionToken::drawn();

        assert_eq!(token.as_str().len(), 64);
        assert!(
            token
                .as_str()
                .bytes()
                .all(|digit| digit.is_ascii_hexdigit())
        );
        assert_ne!(token.as_str(), SessionToken::drawn().as_str());
    }

    #[test]
    fn only_a_digest_of_the_token_is_kept_and_it_is_the_same_each_time() {
        let token = SessionToken::presented("a token the browser sent");

        assert_eq!(token.digest(), token.digest());
        assert_ne!(
            token.digest(),
            SessionToken::presented("another token").digest()
        );
        assert_ne!(
            token.digest().as_str(),
            token.as_str(),
            "a leaked table must not hold sessions anyone could present"
        );
    }

    #[test]
    fn a_digest_is_sha_256_in_hex() {
        assert_eq!(
            SessionToken::presented("abc").digest().as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_token_never_shows_itself_in_a_log() {
        let token = SessionToken::presented("a token the browser sent");

        assert!(!format!("{token:?}").contains("browser"));
    }

    #[test]
    fn a_session_lasts_thirty_days_to_the_second() {
        let session = Session::begun(an_account(), Stage::SignedIn, at(1_000));

        assert!(!session.is_over(at(1_000) + Duration::days(30)));
        assert!(session.is_over(at(1_000) + Duration::days(30) + Duration::seconds(1)));
    }

    #[test]
    fn only_a_session_signed_in_all_the_way_may_act() {
        assert!(Session::begun(an_account(), Stage::SignedIn, at(1_000)).may_act());
        assert!(
            !Session::begun(an_account(), Stage::SecondFactorOwed, at(1_000)).may_act(),
            "a password alone, where a second factor is owed, may do nothing but answer it"
        );
    }
}
