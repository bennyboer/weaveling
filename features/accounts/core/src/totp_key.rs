use std::fmt::{self, Debug, Formatter};

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use thiserror::Error;

use crate::id::AccountId;
use crate::totp::TotpSecret;

/// The server's key that seals every account's TOTP secret, so the stores hold none of them readable.
#[derive(Clone)]
pub struct TotpKey(Aes256Gcm);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("a TOTP key is {} hexadecimal digits", TotpKey::BYTES * 2)]
pub struct InvalidTotpKey;

/// A TOTP secret sealed for one account: the nonce, then the secret and its tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedTotpSecret(Vec<u8>);

impl TotpKey {
    const BYTES: usize = 32;
    const NONCE: usize = 12;

    pub fn from_hex(raw: &str) -> Result<Self, InvalidTotpKey> {
        let digits = raw.trim().as_bytes();
        if digits.len() != Self::BYTES * 2 || !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(InvalidTotpKey);
        }
        let bytes: Vec<u8> = digits
            .chunks(2)
            .map(|pair| (digit(pair[0]) << 4) | digit(pair[1]))
            .collect();

        Aes256Gcm::new_from_slice(&bytes)
            .map(Self)
            .map_err(|_| InvalidTotpKey)
    }

    pub fn seal(&self, secret: &TotpSecret, account: &AccountId) -> SealedTotpSecret {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let sealed = self
            .0
            .encrypt(
                &nonce,
                Payload {
                    msg: secret.as_bytes(),
                    aad: account.as_uuid().as_bytes(),
                },
            )
            .expect("AES-GCM seals a secret of twenty bytes");

        SealedTotpSecret([nonce.as_slice(), &sealed].concat())
    }

    pub fn unseal(&self, sealed: &SealedTotpSecret, account: &AccountId) -> Option<TotpSecret> {
        if sealed.0.len() < Self::NONCE {
            return None;
        }
        let (nonce, sealed) = sealed.0.split_at(Self::NONCE);

        self.0
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: sealed,
                    aad: account.as_uuid().as_bytes(),
                },
            )
            .ok()
            .map(TotpSecret::kept)
    }
}

impl SealedTotpSecret {
    pub fn kept(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Debug for TotpKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("TotpKey(…)")
    }
}

fn digit(hex: u8) -> u8 {
    match hex {
        b'0'..=b'9' => hex - b'0',
        b'a'..=b'f' => hex - b'a' + 10,
        _ => hex - b'A' + 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const OTHER_KEY: &str = "f00102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn key(hex: &str) -> TotpKey {
        TotpKey::from_hex(hex).expect("64 hexadecimal digits")
    }

    fn an_account() -> AccountId {
        AccountId::generate(clock::at(1_000))
    }

    #[test]
    fn a_sealed_secret_is_unsealed_by_its_key_for_its_account() {
        // given: a secret sealed for an account
        let account = an_account();
        let secret = TotpSecret::drawn();
        let sealed = key(KEY).seal(&secret, &account);

        // expect: the same key unseals it for that account
        let unsealed = key(KEY)
            .unseal(&sealed, &account)
            .expect("its key, its account");
        assert_eq!(unsealed.as_bytes(), secret.as_bytes());
        // and: the sealed bytes do not hold the secret
        assert!(
            !sealed
                .as_bytes()
                .windows(20)
                .any(|w| w == secret.as_bytes())
        );
    }

    #[test]
    fn a_sealed_secret_is_not_unsealed_by_another_key_or_for_another_account() {
        // given: a secret sealed for an account
        let account = an_account();
        let sealed = key(KEY).seal(&TotpSecret::drawn(), &account);

        // expect: another key does not unseal it
        assert!(key(OTHER_KEY).unseal(&sealed, &account).is_none());
        // and: neither does its key for another account
        assert!(
            key(KEY)
                .unseal(&sealed, &AccountId::generate(clock::at(2_000)))
                .is_none(),
            "a sealed secret copied to another account must not serve there"
        );
    }

    #[test]
    fn a_sealed_secret_altered_or_cut_short_is_not_unsealed() {
        // given: a secret sealed for an account
        let account = an_account();
        let sealed = key(KEY).seal(&TotpSecret::drawn(), &account);

        // when: one of its bytes is altered
        let mut altered = sealed.as_bytes().to_vec();
        altered[15] ^= 1;
        // then: it is not unsealed
        assert!(
            key(KEY)
                .unseal(&SealedTotpSecret::kept(altered), &account)
                .is_none()
        );
        // and: neither is one shorter than its nonce
        assert!(
            key(KEY)
                .unseal(&SealedTotpSecret::kept(vec![0; 11]), &account)
                .is_none()
        );
    }

    #[test]
    fn the_same_secret_is_sealed_differently_each_time() {
        // given: one secret
        let account = an_account();
        let secret = TotpSecret::drawn();

        // expect: sealed twice, the two differ
        assert_ne!(
            key(KEY).seal(&secret, &account),
            key(KEY).seal(&secret, &account)
        );
    }

    #[test]
    fn a_key_is_sixty_four_hexadecimal_digits() {
        // expect: written in upper case with space around, it is the same key
        let account = an_account();
        let sealed = key(KEY).seal(&TotpSecret::drawn(), &account);
        assert!(
            key(&format!(" {} \n", KEY.to_uppercase()))
                .unseal(&sealed, &account)
                .is_some()
        );
        // and: too few, too many or a digit that is not one are refused
        for wrong in [
            &KEY[2..],
            &format!("{KEY}00"),
            &KEY.replacen('0', "g", 1),
            &KEY.replacen("00", "+0", 1),
            "",
        ] {
            assert_eq!(
                TotpKey::from_hex(wrong).err(),
                Some(InvalidTotpKey),
                "{wrong}"
            );
        }
    }

    #[test]
    fn a_key_is_not_shown_by_debugging() {
        // expect: it stays out of a log
        assert_eq!(format!("{:?}", key(KEY)), "TotpKey(…)");
    }
}
