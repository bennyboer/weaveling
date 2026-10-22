use std::fmt::{self, Debug, Formatter};

use rand_core::{OsRng, RngCore};

use crate::session::{TokenDigest, hex};

pub struct OperatorCode(String);

impl OperatorCode {
    const BYTES: usize = 16;

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
        let code = OperatorCode::drawn();

        assert_eq!(code.as_str().len(), 32);
        assert!(code.as_str().bytes().all(|digit| digit.is_ascii_hexdigit()));
        assert_ne!(code.as_str(), OperatorCode::drawn().as_str());
    }

    #[test]
    fn a_code_copied_from_the_log_with_space_around_it_is_the_same_code() {
        let code = OperatorCode::drawn();

        assert_eq!(
            OperatorCode::presented(&format!("  {}\n", code.as_str())).digest(),
            code.digest()
        );
    }

    #[test]
    fn a_code_is_never_written_out_by_debug() {
        let code = OperatorCode::drawn();

        assert!(!format!("{code:?}").contains(code.as_str()));
    }
}
