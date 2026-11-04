use std::fmt::{self, Debug, Formatter};

use async_trait::async_trait;
use hmac::{Hmac, KeyInit, Mac};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::catalog::CatalogError;
use crate::id::InvitationId;

pub struct InvitationToken(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvitationTokenDigest(String);

#[derive(Clone, PartialEq, Eq)]
pub struct TokenKey([u8; TokenKey::BYTES]);

#[async_trait]
pub trait TokenKeys: Send + Sync {
    async fn key(&self) -> Result<TokenKey, CatalogError>;
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl InvitationToken {
    pub fn derived(key: &TokenKey, invitation: &InvitationId) -> Self {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&key.0).expect("an HMAC takes a key of any length");
        mac.update(invitation.to_string().as_bytes());

        Self(hex(mac.finalize().into_bytes().as_slice()))
    }

    pub fn presented(raw: &str) -> Self {
        Self(raw.trim().to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> InvitationTokenDigest {
        InvitationTokenDigest(hex(Sha256::digest(self.0.as_bytes()).as_slice()))
    }
}

impl Debug for InvitationToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("InvitationToken(…)")
    }
}

impl InvitationTokenDigest {
    pub fn kept(digest: String) -> Self {
        Self(digest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TokenKey {
    pub const BYTES: usize = 32;

    pub fn drawn() -> Self {
        let mut drawn = [0_u8; Self::BYTES];
        OsRng.fill_bytes(&mut drawn);

        Self(drawn)
    }

    pub fn kept(bytes: &[u8]) -> Option<Self> {
        Some(Self(bytes.try_into().ok()?))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Debug for TokenKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("TokenKey(…)")
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn an_invitation() -> InvitationId {
        InvitationId::generate(OffsetDateTime::UNIX_EPOCH)
    }

    #[test]
    fn a_token_is_256_bits_written_in_hex() {
        // given: a token derived for an invitation
        let token = InvitationToken::derived(&TokenKey::drawn(), &an_invitation());

        // then: it is 64 characters long
        assert_eq!(token.as_str().len(), 64);
        // and: all of them hex
        assert!(
            token
                .as_str()
                .bytes()
                .all(|digit| digit.is_ascii_hexdigit())
        );
    }

    #[test]
    fn a_token_derived_twice_is_the_same_token() {
        // given: a key and an invitation
        let (key, invitation) = (TokenKey::drawn(), an_invitation());

        // then: deriving twice gives the same token
        assert_eq!(
            InvitationToken::derived(&key, &invitation).as_str(),
            InvitationToken::derived(&key, &invitation).as_str(),
            "a mail heard again after a crash carries the link the invitation was issued with"
        );
    }

    #[test]
    fn another_invitation_or_another_key_gives_another_token() {
        // given: a token derived with a key for an invitation
        let (key, invitation) = (TokenKey::drawn(), an_invitation());
        let token = InvitationToken::derived(&key, &invitation);

        // then: another invitation gives another token
        assert_ne!(
            InvitationToken::derived(&key, &an_invitation()).as_str(),
            token.as_str()
        );
        // and: another key gives another token
        assert_ne!(
            InvitationToken::derived(&TokenKey::drawn(), &invitation).as_str(),
            token.as_str(),
            "a key drawn anew leaves every link not yet taken up leading nowhere"
        );
    }

    #[test]
    fn only_a_digest_is_kept_the_same_each_time_and_never_the_token() {
        // given: a token
        let token = InvitationToken::derived(&TokenKey::drawn(), &an_invitation());

        // then: its digest is the same each time
        assert_eq!(token.digest(), token.digest());
        // and: is not the token
        assert_ne!(token.digest().as_str(), token.as_str());
        // and: is 64 characters long
        assert_eq!(token.digest().as_str().len(), 64);
    }

    #[test]
    fn a_token_copied_from_a_mail_with_space_around_it_is_the_same_token() {
        // given: a token
        let token = InvitationToken::derived(&TokenKey::drawn(), &an_invitation());

        // then: presented with space around it, it has the same digest
        assert_eq!(
            InvitationToken::presented(&format!(" {}\n", token.as_str())).digest(),
            token.digest()
        );
    }

    #[test]
    fn a_token_is_known_by_the_sha_256_of_it_as_accounts_reckon_it() {
        // expect: a token's digest is its sha-256 as accounts reckon it
        assert_eq!(
            InvitationToken::presented("abc").digest().as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_key_is_32_bytes_and_neither_it_nor_a_token_is_written_out_by_debug() {
        // given: a key and a token derived with it
        let key = TokenKey::drawn();
        let token = InvitationToken::derived(&key, &an_invitation());

        // then: a key is 32 bytes and is kept as it was
        assert!(TokenKey::kept(&[0; 31]).is_none());
        assert_eq!(TokenKey::kept(key.as_bytes()), Some(key.clone()));
        // and: debug writes out neither
        assert!(!format!("{token:?}").contains(token.as_str()));
        assert!(!format!("{key:?}").contains(&hex(key.as_bytes())));
    }
}
