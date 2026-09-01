mod prefixed;

pub use prefixed::InvalidId;
pub use time::{Duration, OffsetDateTime};
pub use uuid::Uuid;

use uuid::{NoContext, Timestamp};

pub fn sortable(now: OffsetDateTime) -> Uuid {
    let seconds = u64::try_from(now.unix_timestamp()).unwrap_or(0);

    Uuid::new_v7(Timestamp::from_unix(NoContext, seconds, now.nanosecond()))
}

const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const WIDTH: usize = 22;

pub fn encode(id: Uuid) -> String {
    let mut left = id.as_u128();
    let mut written = [b'0'; WIDTH];

    for place in written.iter_mut().rev() {
        *place = ALPHABET[(left % 62) as usize];
        left /= 62;
    }

    String::from_utf8(written.to_vec()).expect("the alphabet is ascii")
}

pub fn decode(text: &str) -> Option<Uuid> {
    if text.len() != WIDTH {
        return None;
    }

    let mut built: u128 = 0;
    for character in text.bytes() {
        let digit = ALPHABET.iter().position(|known| *known == character)?;
        built = built.checked_mul(62)?.checked_add(digit as u128)?;
    }

    Some(Uuid::from_u128(built))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_id(seconds: u64) -> Uuid {
        Uuid::from_u128(u128::from(seconds) << 80 | 0x7000_8000_0000_0000_0000)
    }

    #[test]
    fn an_id_survives_the_round_trip() {
        let id = Uuid::from_u128(0x019a_4f2b_4614_74da_b28b_4b88_bbf8_c9f0);

        assert_eq!(decode(&encode(id)), Some(id));
    }

    #[test]
    fn the_smallest_and_largest_ids_survive_too() {
        for edge in [Uuid::from_u128(0), Uuid::from_u128(u128::MAX)] {
            assert_eq!(decode(&encode(edge)), Some(edge), "{edge} should survive");
        }
    }

    #[test]
    fn every_id_encodes_to_the_same_width() {
        for counted in 0..1_000u128 {
            let encoded = encode(Uuid::from_u128(counted * 7_919_191_919_191_919));

            assert_eq!(encoded.len(), WIDTH, "{encoded} is the wrong width");
        }
    }

    #[test]
    fn the_smallest_id_is_padded_rather_than_empty() {
        assert_eq!(encode(Uuid::from_u128(0)), "0".repeat(WIDTH));
    }

    #[test]
    fn text_order_follows_id_order() {
        let mut ids: Vec<Uuid> = (0..500u64)
            .map(|counted| an_id(counted * 1_000))
            .rev()
            .collect();
        ids.sort();

        let mut encoded: Vec<String> = ids.iter().map(|id| encode(*id)).collect();
        let sorted_as_ids = encoded.clone();
        encoded.sort();

        assert_eq!(
            encoded, sorted_as_ids,
            "a store that sorts the text form must get the same order as one sorting the id"
        );
    }

    #[test]
    fn an_encoded_id_is_shorter_than_the_hyphenated_form() {
        let id = Uuid::from_u128(0x019a_4f2b_4614_74da_b28b_4b88_bbf8_c9f0);

        assert!(
            encode(id).len() < id.to_string().len(),
            "the whole point is brevity"
        );
    }

    #[test]
    fn nonsense_does_not_decode() {
        assert_eq!(decode(""), None);
        assert_eq!(decode("too-short"), None);
        assert_eq!(decode(&"x".repeat(WIDTH + 1)), None);
        assert_eq!(decode(&"!".repeat(WIDTH)), None);
    }

    #[test]
    fn a_hyphenated_uuid_does_not_decode() {
        let hyphenated = Uuid::from_u128(0x019a_4f2b_4614_74da_b28b_4b88_bbf8_c9f0).to_string();

        assert_eq!(decode(&hyphenated), None);
    }

    #[test]
    fn a_value_beyond_the_range_does_not_decode() {
        let beyond = "z".repeat(WIDTH);

        assert_eq!(
            decode(&beyond),
            None,
            "22 base62 digits can express more than 128 bits, and the excess must be refused"
        );
    }

    #[test]
    fn the_alphabet_is_in_ascending_order() {
        let mut sorted = *ALPHABET;
        sorted.sort_unstable();

        assert_eq!(
            sorted, *ALPHABET,
            "text ordering only tracks id ordering while the alphabet ascends"
        );
    }
}
