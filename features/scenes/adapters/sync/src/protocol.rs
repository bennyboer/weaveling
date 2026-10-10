use yrs::encoding::read::{Cursor, Error, Read};
use yrs::encoding::write::Write;

const SYNC: u64 = 0;
const AWARENESS: u64 = 1;
const QUERY_AWARENESS: u64 = 3;

const STEP1: u64 = 0;
const STEP2: u64 = 1;
const UPDATE: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    WhatDoYouHave(Vec<u8>),
    HereIsWhatYouMissed(Vec<u8>),
    JustHappened(Vec<u8>),
    Awareness(Vec<u8>),
    WhoIsHere,
}

impl Message {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        match self {
            Message::WhatDoYouHave(state_vector) => {
                out.write_var(SYNC);
                out.write_var(STEP1);
                out.write_buf(state_vector);
            }
            Message::HereIsWhatYouMissed(update) => {
                out.write_var(SYNC);
                out.write_var(STEP2);
                out.write_buf(update);
            }
            Message::JustHappened(update) => {
                out.write_var(SYNC);
                out.write_var(UPDATE);
                out.write_buf(update);
            }
            Message::Awareness(payload) => {
                out.write_var(AWARENESS);
                out.write_buf(payload);
            }
            Message::WhoIsHere => {
                out.write_var(QUERY_AWARENESS);
            }
        }

        out
    }

    pub fn decode(frame: &[u8]) -> Result<Message, Error> {
        let mut cursor = Cursor::new(frame);

        match cursor.read_var::<u64>()? {
            SYNC => match cursor.read_var::<u64>()? {
                STEP1 => Ok(Message::WhatDoYouHave(cursor.read_buf()?.to_vec())),
                STEP2 => Ok(Message::HereIsWhatYouMissed(cursor.read_buf()?.to_vec())),
                UPDATE => Ok(Message::JustHappened(cursor.read_buf()?.to_vec())),
                _ => Err(Error::UnexpectedValue),
            },
            AWARENESS => Ok(Message::Awareness(cursor.read_buf()?.to_vec())),
            QUERY_AWARENESS => Ok(Message::WhoIsHere),
            _ => Err(Error::UnexpectedValue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(message: Message) {
        assert_eq!(
            Message::decode(&message.encode()).expect("our own frame should decode"),
            message
        );
    }

    #[test]
    fn every_message_survives_a_round_trip() {
        round_trip(Message::WhatDoYouHave(vec![0, 1, 2]));
        round_trip(Message::HereIsWhatYouMissed(vec![9, 8, 7]));
        round_trip(Message::JustHappened(vec![4]));
        round_trip(Message::Awareness(vec![1, 2, 3, 4, 5]));
        round_trip(Message::WhoIsHere);
    }

    #[test]
    fn frames_match_the_y_protocols_wire_format() {
        assert_eq!(
            Message::WhatDoYouHave(vec![7, 7]).encode(),
            vec![0, 0, 2, 7, 7]
        );
        assert_eq!(
            Message::HereIsWhatYouMissed(vec![7]).encode(),
            vec![0, 1, 1, 7]
        );
        assert_eq!(Message::JustHappened(vec![7]).encode(), vec![0, 2, 1, 7]);
        assert_eq!(Message::Awareness(vec![7]).encode(), vec![1, 1, 7]);
        assert_eq!(Message::WhoIsHere.encode(), vec![3]);
    }

    #[test]
    fn a_truncated_frame_is_an_error_not_a_panic() {
        assert!(Message::decode(&[]).is_err());
        assert!(Message::decode(&[0]).is_err());
        assert!(Message::decode(&[0, 0]).is_err());
        assert!(Message::decode(&[0, 0, 9, 1]).is_err());
        assert!(Message::decode(&[99]).is_err());
    }
}
