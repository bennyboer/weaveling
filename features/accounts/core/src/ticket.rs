use std::fmt::{self, Debug, Formatter};

use time::OffsetDateTime;

use crate::session::TokenDigest;
use email::EmailAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationTicket {
    pub invitation: String,
    pub to: EmailAddress,
    pub until: OffsetDateTime,
}

pub struct InvitationToken(String);

impl InvitationToken {
    pub fn presented(raw: &str) -> Self {
        Self(raw.trim().to_owned())
    }

    pub fn digest(&self) -> TokenDigest {
        TokenDigest::of(&self.0)
    }
}

impl Debug for InvitationToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("InvitationToken(…)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_known_by_the_sha_256_of_it_as_invitations_keep_it() {
        assert_eq!(
            InvitationToken::presented(" abc\n").digest(),
            TokenDigest::kept(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned()
            ),
            "invitations tell the digest and accounts reckon it again from the link, so both must reckon alike"
        );
    }

    #[test]
    fn a_token_is_never_written_out_by_debug() {
        assert!(!format!("{:?}", InvitationToken::presented("abc")).contains("abc"));
    }
}
