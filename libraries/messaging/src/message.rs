use std::fmt::{self, Display, Formatter};

use serde_json::Value;
use time::OffsetDateTime;
use uuid::{NoContext, Timestamp, Uuid};

use crate::routing::RoutingKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Conversation(MessageId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub routing: RoutingKey,
    pub conversation: Conversation,
    pub caused_by: Option<MessageId>,
    pub occurred_at: OffsetDateTime,
    pub payload: Value,
}

impl MessageId {
    pub fn generate(now: OffsetDateTime) -> Self {
        let seconds = u64::try_from(now.unix_timestamp()).unwrap_or(0);

        Self(Uuid::new_v7(Timestamp::from_unix(
            NoContext,
            seconds,
            now.nanosecond(),
        )))
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Display for MessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "message_{}", ids::encode(self.0))
    }
}

impl Conversation {
    pub fn begun_by(first: MessageId) -> Self {
        Self(first)
    }

    pub fn as_message_id(self) -> MessageId {
        self.0
    }
}

impl Display for Conversation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Message {
    pub fn opening(routing: RoutingKey, payload: Value, now: OffsetDateTime) -> Self {
        let id = MessageId::generate(now);

        Self {
            id,
            routing,
            conversation: Conversation::begun_by(id),
            caused_by: None,
            occurred_at: now,
            payload,
        }
    }

    pub fn answering(&self, routing: RoutingKey, payload: Value, now: OffsetDateTime) -> Self {
        Self {
            id: MessageId::generate(now),
            routing,
            conversation: self.conversation,
            caused_by: Some(self.id),
            occurred_at: now,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::Duration;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_key(of: &str) -> RoutingKey {
        RoutingKey::parse(of).expect("a plain key is fine")
    }

    fn an_opening_message() -> Message {
        Message::opening(
            a_key("piece.captured"),
            json!({ "piece": "piece_1" }),
            at(1_000),
        )
    }

    #[test]
    fn a_message_that_starts_a_conversation_is_its_own_beginning() {
        let opening = an_opening_message();

        assert_eq!(opening.conversation.as_message_id(), opening.id);
        assert_eq!(
            opening.caused_by, None,
            "nothing came before the first message"
        );
    }

    #[test]
    fn an_answer_stays_in_the_conversation_it_answers() {
        let asked = an_opening_message();

        let answered = asked.answering(a_key("board.pinned"), json!({}), at(1_001));

        assert_eq!(answered.conversation, asked.conversation);
        assert_ne!(answered.id, asked.id);
    }

    #[test]
    fn an_answer_records_what_provoked_it() {
        let asked = an_opening_message();

        let answered = asked.answering(a_key("board.pinned"), json!({}), at(1_001));

        assert_eq!(answered.caused_by, Some(asked.id));
    }

    #[test]
    fn a_conversation_survives_more_than_one_hop() {
        let first = an_opening_message();
        let second = first.answering(a_key("board.pinned"), json!({}), at(1_001));

        let third = second.answering(a_key("piece.settled"), json!({}), at(1_002));

        assert_eq!(
            third.conversation, first.conversation,
            "everything a request set off must be traceable to that request"
        );
        assert_eq!(
            third.caused_by,
            Some(second.id),
            "but each hop still names the one directly before it"
        );
    }

    #[test]
    fn two_messages_made_at_the_same_moment_still_differ() {
        assert_ne!(
            MessageId::generate(at(1_000)),
            MessageId::generate(at(1_000))
        );
    }

    #[test]
    fn message_ids_sort_by_the_moment_they_were_made() {
        let latest = MessageId::generate(at(3_000));
        let earliest = MessageId::generate(at(1_000));

        let mut made = vec![latest, earliest];
        made.sort();

        assert_eq!(made, vec![earliest, latest]);
    }

    #[test]
    fn a_message_id_says_what_it_is() {
        let id = MessageId::generate(at(1_000));

        assert!(
            id.to_string().starts_with("message_"),
            "an id should say what it is: {id}"
        );
    }

    #[test]
    fn a_message_carries_what_it_was_given() {
        let opening = an_opening_message();

        assert_eq!(opening.routing, a_key("piece.captured"));
        assert_eq!(opening.payload, json!({ "piece": "piece_1" }));
        assert_eq!(opening.occurred_at, at(1_000));
    }
}
