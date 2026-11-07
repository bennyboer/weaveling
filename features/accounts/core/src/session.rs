use std::fmt::{self, Debug, Formatter};

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
        getrandom::fill(&mut drawn).expect("the system draws random bytes");

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
    pub const OWING_LASTS: Duration = Duration::minutes(10);

    pub fn begun(account: AccountId, stage: Stage, now: OffsetDateTime) -> Self {
        Self {
            account,
            stage,
            begun: now,
            until: now
                + match stage {
                    Stage::SignedIn => Self::LASTS,
                    Stage::SecondFactorOwed => Self::OWING_LASTS,
                },
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

    fn an_account() -> AccountId {
        AccountId::generate(clock::at(1_000))
    }

    #[test]
    fn a_token_is_256_random_bits_written_in_hex() {
        // when: a token is drawn
        let token = SessionToken::drawn();

        // then: it is 64 hex digits
        assert_eq!(token.as_str().len(), 64);
        assert!(
            token
                .as_str()
                .bytes()
                .all(|digit| digit.is_ascii_hexdigit())
        );
        // and: another token drawn is another
        assert_ne!(token.as_str(), SessionToken::drawn().as_str());
    }

    #[test]
    fn only_a_digest_of_the_token_is_kept_and_it_is_the_same_each_time() {
        // given: a presented token
        let token = SessionToken::presented("a token the browser sent");

        // then: its digest is the same each time
        assert_eq!(token.digest(), token.digest());
        // and: another token's digest is another
        assert_ne!(
            token.digest(),
            SessionToken::presented("another token").digest()
        );
        // and: the digest is not the token
        assert_ne!(
            token.digest().as_str(),
            token.as_str(),
            "a leaked table must not hold sessions anyone could present"
        );
    }

    #[test]
    fn a_digest_is_sha_256_in_hex() {
        // expect: the digest of a token is its sha 256 in hex
        assert_eq!(
            SessionToken::presented("abc").digest().as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_token_never_shows_itself_in_a_log() {
        // given: a presented token
        let token = SessionToken::presented("a token the browser sent");

        // expect: its debug form does not hold it
        assert!(!format!("{token:?}").contains("browser"));
    }

    #[test]
    fn a_session_lasts_thirty_days_to_the_second() {
        // given: a session begun
        let session = Session::begun(an_account(), Stage::SignedIn, clock::at(1_000));

        // then: it holds for thirty days
        assert!(!session.is_over(clock::at(1_000) + Duration::days(30)));
        // and: is over a second later
        assert!(session.is_over(clock::at(1_000) + Duration::days(30) + Duration::seconds(1)));
    }

    #[test]
    fn a_session_owing_its_second_factor_lasts_ten_minutes() {
        // given: a session owing its second factor
        let session = Session::begun(an_account(), Stage::SecondFactorOwed, clock::at(1_000));

        // then: it holds for ten minutes
        assert!(!session.is_over(clock::at(1_000) + Duration::minutes(10)));
        // and: is over a second later
        assert!(
            session.is_over(clock::at(1_000) + Duration::minutes(10) + Duration::seconds(1)),
            "a password alone keeps the door ajar only while the code is being typed"
        );
    }

    #[test]
    fn only_a_session_signed_in_all_the_way_may_act() {
        // expect: a session signed in may act
        assert!(Session::begun(an_account(), Stage::SignedIn, clock::at(1_000)).may_act());
        // and: one still owing a second factor may not
        assert!(
            !Session::begun(an_account(), Stage::SecondFactorOwed, clock::at(1_000)).may_act(),
            "a password alone, where a second factor is owed, may do nothing but answer it"
        );
    }
}
