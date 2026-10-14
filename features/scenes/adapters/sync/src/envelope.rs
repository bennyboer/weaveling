use yrs::encoding::read::{Cursor, Error, Read};
use yrs::encoding::write::Write;

const JOIN: u64 = 0;
const LEAVE: u64 = 1;
const CARRY: u64 = 2;
const REFUSED: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Envelope {
    Join(String),
    Leave(String),
    Carry { scene: String, frame: Vec<u8> },
    Refused(String),
}

impl Envelope {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        match self {
            Envelope::Join(scene) => {
                out.write_var(JOIN);
                out.write_string(scene);
            }
            Envelope::Leave(scene) => {
                out.write_var(LEAVE);
                out.write_string(scene);
            }
            Envelope::Carry { scene, frame } => {
                out.write_var(CARRY);
                out.write_string(scene);
                out.write_buf(frame);
            }
            Envelope::Refused(scene) => {
                out.write_var(REFUSED);
                out.write_string(scene);
            }
        }

        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Envelope, Error> {
        let mut cursor = Cursor::new(bytes);
        let kind = cursor.read_var::<u64>()?;
        let scene = cursor.read_string()?.to_owned();

        match kind {
            JOIN => Ok(Envelope::Join(scene)),
            LEAVE => Ok(Envelope::Leave(scene)),
            CARRY => Ok(Envelope::Carry {
                scene,
                frame: cursor.read_buf()?.to_vec(),
            }),
            REFUSED => Ok(Envelope::Refused(scene)),
            _ => Err(Error::UnexpectedValue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(envelope: Envelope) {
        assert_eq!(
            Envelope::decode(&envelope.encode()).expect("our own envelope should decode"),
            envelope
        );
    }

    #[test]
    fn every_envelope_survives_a_round_trip() {
        round_trip(Envelope::Join("scene_1".to_owned()));
        round_trip(Envelope::Leave("scene_1".to_owned()));
        round_trip(Envelope::Carry {
            scene: "scene_1".to_owned(),
            frame: vec![0, 1, 2],
        });
        round_trip(Envelope::Refused("scene_1".to_owned()));
    }

    #[test]
    fn an_envelope_is_a_kind_a_scene_and_maybe_a_frame_in_lib0_encoding() {
        assert_eq!(
            Envelope::Carry {
                scene: "ab".to_owned(),
                frame: vec![7, 7],
            }
            .encode(),
            vec![2, 2, b'a', b'b', 2, 7, 7],
            "the browser writes these with lib0, so the bytes are the contract"
        );
        assert_eq!(
            Envelope::Join("ab".to_owned()).encode(),
            vec![0, 2, b'a', b'b']
        );
    }

    #[test]
    fn a_truncated_or_unknown_envelope_is_an_error_not_a_panic() {
        assert!(Envelope::decode(&[]).is_err());
        assert!(Envelope::decode(&[2]).is_err());
        assert!(Envelope::decode(&[2, 2, b'a', b'b']).is_err());
        assert!(Envelope::decode(&[9, 1, b'a']).is_err());
    }
}
