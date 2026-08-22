use passages_core::{Passage, PassageError, PassageId};
use thiserror::Error;

use crate::protocol::Message;

#[derive(Debug, Error)]
pub enum RoomError {
    #[error("a peer sent a frame we could not read: {0}")]
    Frame(#[from] yrs::encoding::read::Error),

    #[error(transparent)]
    Passage(#[from] PassageError),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    pub to_sender: Option<Message>,
    pub to_others: Option<Message>,
    pub to_store: Option<Vec<u8>>,
}

pub struct Room {
    passage: Passage,
}

impl Room {
    pub fn adopt(passage: Passage) -> Self {
        Room { passage }
    }

    pub fn id(&self) -> PassageId {
        self.passage.id()
    }

    pub fn text(&self) -> String {
        self.passage.text()
    }

    pub fn greet(&self) -> Reaction {
        Reaction {
            to_sender: Some(Message::WhatDoYouHave(self.passage.state_vector())),
            to_others: Some(Message::WhoIsHere),
            ..Reaction::default()
        }
    }

    pub fn receive(&self, message: Message) -> Result<Reaction, RoomError> {
        match message {
            Message::WhatDoYouHave(state_vector) => Ok(Reaction {
                to_sender: Some(Message::HereIsWhatYouMissed(
                    self.passage.changes_since(&state_vector)?,
                )),
                ..Reaction::default()
            }),
            Message::HereIsWhatYouMissed(update) | Message::JustHappened(update) => {
                self.passage.absorb(&update)?;

                Ok(Reaction {
                    to_others: Some(Message::JustHappened(update.clone())),
                    to_store: Some(update),
                    ..Reaction::default()
                })
            }
            Message::Awareness(payload) => Ok(Reaction {
                to_others: Some(Message::Awareness(payload)),
                ..Reaction::default()
            }),
            Message::WhoIsHere => Ok(Reaction {
                to_others: Some(Message::WhoIsHere),
                ..Reaction::default()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use time::{Duration, OffsetDateTime};
    use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

    use super::*;

    fn an_id(seconds: i64) -> PassageId {
        PassageId::generate(OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds))
    }

    fn a_paragraph(saying: &str) -> Vec<u8> {
        let doc = Doc::new();
        let fragment = doc.get_or_insert_xml_fragment(passages_core::FRAGMENT);
        {
            let mut txn = doc.transact_mut();
            let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
            paragraph.insert(&mut txn, 0, XmlTextPrelim::new(saying));
        }

        doc.transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    fn an_empty_room() -> Room {
        Room::adopt(Passage::empty(an_id(1_000)))
    }

    fn deliver(room: &Room, message: Message) -> Reaction {
        room.receive(message).expect("the room should accept this")
    }

    #[test]
    fn a_greeting_publishes_what_the_room_already_has() {
        let room = an_empty_room();
        deliver(
            &room,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );

        let Some(Message::WhatDoYouHave(state_vector)) = room.greet().to_sender else {
            panic!("a greeting should ask the newcomer for their state");
        };

        assert!(
            !state_vector.is_empty(),
            "the room should publish its own state"
        );
    }

    #[test]
    fn a_greeting_also_asks_the_others_to_republish_their_cursors() {
        let room = an_empty_room();

        let reaction = room.greet();

        assert_eq!(
            reaction.to_others,
            Some(Message::WhoIsHere),
            "without this a newcomer sees no cursors until someone moves"
        );
        assert_eq!(reaction.to_store, None, "joining is not a write");
    }

    #[test]
    fn a_newcomer_asking_what_we_have_is_told_everything() {
        let room = an_empty_room();
        deliver(
            &room,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );
        let newcomer = Passage::empty(an_id(2_000));

        let reaction = deliver(&room, Message::WhatDoYouHave(newcomer.state_vector()));

        let Some(Message::HereIsWhatYouMissed(catch_up)) = reaction.to_sender else {
            panic!("expected a catch-up update, got {:?}", reaction.to_sender);
        };
        newcomer.absorb(&catch_up).expect("catch-up should apply");
        assert_eq!(newcomer.text(), "The loom stood silent.");
    }

    #[test]
    fn catching_a_newcomer_up_tells_nobody_else_and_stores_nothing() {
        let room = an_empty_room();

        let reaction = deliver(
            &room,
            Message::WhatDoYouHave(Passage::empty(an_id(2)).state_vector()),
        );

        assert_eq!(reaction.to_others, None);
        assert_eq!(reaction.to_store, None, "a read must not write");
    }

    #[test]
    fn an_edit_reaches_the_other_peers_and_the_store_but_is_not_echoed_back() {
        let room = an_empty_room();
        let update = a_paragraph("The loom stood silent.");

        let reaction = deliver(&room, Message::JustHappened(update.clone()));

        assert_eq!(
            reaction.to_sender, None,
            "a peer must not receive its own edit"
        );
        assert_eq!(
            reaction.to_others,
            Some(Message::JustHappened(update.clone()))
        );
        assert_eq!(
            reaction.to_store,
            Some(update),
            "an edit must be handed to the store verbatim"
        );
    }

    #[test]
    fn the_room_holds_the_document_itself_not_just_a_pipe() {
        let room = an_empty_room();

        deliver(
            &room,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );

        assert_eq!(
            room.text(),
            "The loom stood silent.",
            "the room must be able to serve a peer who was never connected"
        );
    }

    #[test]
    fn a_catch_up_from_a_peer_is_absorbed_like_any_other_edit() {
        let room = an_empty_room();

        let reaction = deliver(
            &room,
            Message::HereIsWhatYouMissed(a_paragraph("The loom stood silent.")),
        );

        assert_eq!(room.text(), "The loom stood silent.");
        assert!(reaction.to_store.is_some(), "it is still durable prose");
    }

    #[test]
    fn awareness_is_relayed_without_being_understood_or_stored() {
        let room = an_empty_room();
        let nonsense = vec![200, 13, 42, 7];

        let reaction = deliver(&room, Message::Awareness(nonsense.clone()));

        assert_eq!(reaction.to_others, Some(Message::Awareness(nonsense)));
        assert_eq!(reaction.to_sender, None);
        assert_eq!(
            reaction.to_store, None,
            "presence must never reach the store"
        );
        assert_eq!(room.text(), "", "awareness must not touch the document");
    }

    #[test]
    fn asking_who_is_here_reaches_the_other_peers() {
        let room = an_empty_room();

        let reaction = deliver(&room, Message::WhoIsHere);

        assert_eq!(reaction.to_others, Some(Message::WhoIsHere));
        assert_eq!(reaction.to_store, None);
    }

    #[test]
    fn an_unusable_update_is_refused_and_changes_nothing() {
        let room = an_empty_room();
        deliver(
            &room,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );

        let outcome = room.receive(Message::JustHappened(vec![255, 255, 255, 255]));

        assert!(outcome.is_err(), "garbage must not be accepted");
        assert_eq!(
            room.text(),
            "The loom stood silent.",
            "a refused update must leave the room as it was"
        );
    }

    #[test]
    fn a_corrupt_state_vector_is_refused() {
        let room = an_empty_room();

        assert!(
            room.receive(Message::WhatDoYouHave(vec![255, 255, 255, 255]))
                .is_err()
        );
    }
}
