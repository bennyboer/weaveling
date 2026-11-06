use std::fmt::{self, Display, Formatter};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
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

    pub fn of(known: Uuid) -> Self {
        Self(known)
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

#[derive(Debug, Error)]
#[error("this {routing} message is not shaped as it should be: {because}")]
pub struct Unshaped {
    routing: RoutingKey,
    #[source]
    because: Misshapen,
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct Misshapen(String);

impl From<serde_json::Error> for Misshapen {
    fn from(read: serde_json::Error) -> Self {
        Self(without_values(&read.to_string()))
    }
}

fn without_values(said: &str) -> String {
    let mut kept = String::with_capacity(said.len());
    let mut quoting = false;
    let mut escaping = false;

    for letter in said.chars() {
        if !quoting {
            kept.push(letter);
            quoting = letter == '"';
        } else if escaping {
            escaping = false;
        } else if letter == '\\' {
            escaping = true;
        } else if letter == '"' {
            kept.push_str("…\"");
            quoting = false;
        }
    }

    kept
}

impl Message {
    pub fn payload_as<T: DeserializeOwned>(&self) -> Result<T, Unshaped> {
        serde_json::from_value(self.payload.clone()).map_err(|because| Unshaped {
            routing: self.routing.clone(),
            because: because.into(),
        })
    }

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

    pub fn follow_up(&self, routing: &str, payload: &impl Serialize, now: OffsetDateTime) -> Self {
        self.answering(
            RoutingKey::parse(routing).expect("a follow-up's routing key holds no wildcards"),
            serde_json::to_value(payload)
                .expect("a follow-up is plain data and cannot fail to serialize"),
            now,
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn a_key(of: &str) -> RoutingKey {
        RoutingKey::parse(of).expect("a plain key is fine")
    }

    fn an_opening_message() -> Message {
        Message::opening(
            a_key("idea.captured"),
            json!({ "idea": "idea_1" }),
            clock::at(1_000),
        )
    }

    #[test]
    fn a_message_that_starts_a_conversation_is_its_own_beginning() {
        // when: a message opens a conversation
        let opening = an_opening_message();

        // then: the conversation begins with it
        assert_eq!(opening.conversation.as_message_id(), opening.id);
        // and: nothing caused it
        assert_eq!(
            opening.caused_by, None,
            "nothing came before the first message"
        );
    }

    #[test]
    fn an_answer_stays_in_the_conversation_it_answers() {
        // given: an opening message
        let asked = an_opening_message();

        // when: it is answered
        let answered = asked.answering(a_key("board.pinned"), json!({}), clock::at(1_001));

        // then: the answer is in its conversation
        assert_eq!(answered.conversation, asked.conversation);
        // and: is a message of its own
        assert_ne!(answered.id, asked.id);
    }

    #[test]
    fn an_answer_records_what_provoked_it() {
        // given: an opening message
        let asked = an_opening_message();

        // when: it is answered
        let answered = asked.answering(a_key("board.pinned"), json!({}), clock::at(1_001));

        // then: the answer names it as its cause
        assert_eq!(answered.caused_by, Some(asked.id));
    }

    #[test]
    fn a_follow_up_answers_what_it_follows_with_its_key_and_its_data() {
        // given: an opening message
        let asked = an_opening_message();

        // when: it is followed up
        let followed = asked.follow_up("follow-up.board.sweep", &["idea_1"], clock::at(1_001));

        // then: the follow-up answers it
        assert_eq!(followed.caused_by, Some(asked.id));
        assert_eq!(followed.conversation, asked.conversation);
        // and: carries its key, its data and its moment
        assert_eq!(followed.routing, a_key("follow-up.board.sweep"));
        assert_eq!(followed.payload, json!(["idea_1"]));
        assert_eq!(followed.occurred_at, clock::at(1_001));
    }

    #[test]
    fn a_conversation_survives_more_than_one_hop() {
        // given: an opening message and an answer to it
        let first = an_opening_message();
        let second = first.answering(a_key("board.pinned"), json!({}), clock::at(1_001));

        // when: the answer is answered
        let third = second.answering(a_key("idea.settled"), json!({}), clock::at(1_002));

        // then: the third is still in the first's conversation
        assert_eq!(
            third.conversation, first.conversation,
            "everything a request set off must be traceable to that request"
        );
        // and: names the second as its cause
        assert_eq!(
            third.caused_by,
            Some(second.id),
            "but each hop still names the one directly before it"
        );
    }

    #[test]
    fn two_messages_made_at_the_same_moment_still_differ() {
        // expect: two ids made at one moment differ
        assert_ne!(
            MessageId::generate(clock::at(1_000)),
            MessageId::generate(clock::at(1_000))
        );
    }

    #[test]
    fn message_ids_sort_by_the_moment_they_were_made() {
        // given: an id made late and one made early
        let latest = MessageId::generate(clock::at(3_000));
        let earliest = MessageId::generate(clock::at(1_000));

        // when: they are sorted
        let mut made = vec![latest, earliest];
        made.sort();

        // then: the early one comes first
        assert_eq!(made, vec![earliest, latest]);
    }

    #[test]
    fn a_message_id_says_what_it_is() {
        // when: an id is made
        let id = MessageId::generate(clock::at(1_000));

        // then: it says it is a message's
        assert!(
            id.to_string().starts_with("message_"),
            "an id should say what it is: {id}"
        );
    }

    #[test]
    fn a_message_carries_what_it_was_given() {
        // when: a message is made
        let opening = an_opening_message();

        // then: it carries its key, its payload and its moment
        assert_eq!(opening.routing, a_key("idea.captured"));
        assert_eq!(opening.payload, json!({ "idea": "idea_1" }));
        assert_eq!(opening.occurred_at, clock::at(1_000));
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Captured {
        idea: String,
    }

    #[test]
    fn a_message_is_read_in_the_shape_it_was_sent_in() {
        // expect: a message reads back in the shape it was sent in
        assert_eq!(
            an_opening_message()
                .payload_as::<Captured>()
                .expect("a capture carries its idea"),
            Captured {
                idea: "idea_1".to_owned()
            }
        );
    }

    #[derive(Debug, serde::Deserialize)]
    struct Counted {
        #[serde(rename = "idea")]
        _words: u32,
    }

    #[test]
    fn a_message_of_another_shape_says_how_but_never_what_it_held() {
        // when: a message is read in a shape it does not have
        let unshaped = Message::opening(
            a_key("account.signed.up"),
            json!({ "idea": "Ada \"the first\" Lovelace" }),
            clock::at(1_000),
        )
        .payload_as::<Counted>()
        .expect_err("a name is no count");

        // then: the refusal leaves out what it held
        let told = format!("{unshaped}: {}", unshaped.because);
        assert!(
            !told.contains("Ada") && !told.contains("first") && !told.contains("Lovelace"),
            "a refusal is shown and kept, so what a message held stays out of it: {told}"
        );
        // and: says how it differs
        assert!(
            told.contains("invalid type: string \"…\", expected u32"),
            "{told}"
        );
    }

    #[test]
    fn what_was_missing_is_still_named() {
        // when: a message is read in a shape whose field it lacks
        let unshaped = Message::opening(a_key("idea.captured"), json!({}), clock::at(1_000))
            .payload_as::<Counted>()
            .expect_err("nothing is no count");

        // then: the refusal names the missing field
        assert!(
            unshaped.to_string().contains("missing field `idea`"),
            "{unshaped}"
        );
    }

    #[test]
    fn a_message_of_another_shape_says_which_message_it_was() {
        // when: a message is read in a shape it does not have
        let unshaped = an_opening_message()
            .payload_as::<u32>()
            .expect_err("a capture is no number");

        // then: the refusal names the message by its key
        assert!(
            unshaped
                .to_string()
                .starts_with("this idea.captured message is not shaped as it should be"),
            "{unshaped}"
        );
    }
}
