use std::fmt::{self, Debug, Formatter};

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

pub struct InvitationToken(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvitationTokenDigest(String);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl InvitationToken {
    const BYTES: usize = 32;

    pub fn drawn() -> Self {
        let mut drawn = [0_u8; Self::BYTES];
        OsRng.fill_bytes(&mut drawn);

        Self(hex(&drawn))
    }

    pub fn presented(raw: &str) -> Self {
        Self(raw.trim().to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> InvitationTokenDigest {
        InvitationTokenDigest(hex(&Sha256::digest(self.0.as_bytes())))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_256_random_bits_written_in_hex() {
        let token = InvitationToken::drawn();

        assert_eq!(token.as_str().len(), 64);
        assert!(
            token
                .as_str()
                .bytes()
                .all(|digit| digit.is_ascii_hexdigit())
        );
        assert_ne!(token.as_str(), InvitationToken::drawn().as_str());
    }

    #[test]
    fn only_a_digest_is_kept_the_same_each_time_and_never_the_token() {
        let token = InvitationToken::drawn();

        assert_eq!(token.digest(), token.digest());
        assert_ne!(token.digest().as_str(), token.as_str());
        assert_eq!(token.digest().as_str().len(), 64);
        assert_ne!(token.digest(), InvitationToken::drawn().digest());
    }

    #[test]
    fn a_token_copied_from_a_mail_with_space_around_it_is_the_same_token() {
        let token = InvitationToken::drawn();

        assert_eq!(
            InvitationToken::presented(&format!(" {}\n", token.as_str())).digest(),
            token.digest()
        );
    }

    #[test]
    fn a_token_is_never_written_out_by_debug() {
        let token = InvitationToken::drawn();

        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
