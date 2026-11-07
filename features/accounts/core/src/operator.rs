use std::fmt::{self, Debug, Formatter};

use crate::session;
use crate::session::TokenDigest;

pub struct OperatorCode(String);

impl OperatorCode {
    const BYTES: usize = 16;

    pub fn drawn() -> Self {
        let mut drawn = [0_u8; Self::BYTES];
        getrandom::fill(&mut drawn).expect("the system draws random bytes");

        Self(session::hex(&drawn))
    }

    pub fn presented(raw: &str) -> Self {
        Self(raw.trim().to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> TokenDigest {
        TokenDigest::of(&self.0)
    }
}

impl Debug for OperatorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("OperatorCode(…)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_128_random_bits_written_in_hex() {
        // when: a code is drawn
        let code = OperatorCode::drawn();

        // then: it is 32 hex digits
        assert_eq!(code.as_str().len(), 32);
        assert!(code.as_str().bytes().all(|digit| digit.is_ascii_hexdigit()));
        // and: another code drawn is another
        assert_ne!(code.as_str(), OperatorCode::drawn().as_str());
    }

    #[test]
    fn a_code_copied_from_the_log_with_space_around_it_is_the_same_code() {
        // given: a drawn code
        let code = OperatorCode::drawn();

        // expect: it presented with space around it has the same digest
        assert_eq!(
            OperatorCode::presented(&format!("  {}\n", code.as_str())).digest(),
            code.digest()
        );
    }

    #[test]
    fn a_code_is_never_written_out_by_debug() {
        // given: a drawn code
        let code = OperatorCode::drawn();

        // expect: its debug form does not hold it
        assert!(!format!("{code:?}").contains(code.as_str()));
    }
}
