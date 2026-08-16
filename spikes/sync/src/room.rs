use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

use crate::protocol::Message;

#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("a peer sent a frame we could not read: {0}")]
    Unreadable(#[from] yrs::encoding::read::Error),

    #[error("a peer sent an update we could not apply: {0}")]
    Unusable(#[from] yrs::error::UpdateError),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    pub to_sender: Option<Message>,
    pub to_others: Option<Message>,
}

impl Reaction {
    fn answer(message: Message) -> Self {
        Reaction {
            to_sender: Some(message),
            to_others: None,
        }
    }

    fn pass_on(message: Message) -> Self {
        Reaction {
            to_sender: None,
            to_others: Some(message),
        }
    }
}

pub struct Room {
    doc: Doc,
}

impl Default for Room {
    fn default() -> Self {
        Room::new()
    }
}

impl Room {
    pub fn new() -> Self {
        Room { doc: Doc::new() }
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    pub fn greet(&self) -> Message {
        Message::WhatDoYouHave(self.doc.transact().state_vector().encode_v1())
    }

    pub fn receive(&self, message: Message) -> Result<Reaction, RoomError> {
        match message {
            Message::WhatDoYouHave(state_vector) => {
                let theirs = StateVector::decode_v1(&state_vector)?;
                let missed = self.doc.transact().encode_state_as_update_v1(&theirs);

                Ok(Reaction::answer(Message::HereIsWhatYouMissed(missed)))
            }
            Message::HereIsWhatYouMissed(update) | Message::JustHappened(update) => {
                self.doc
                    .transact_mut()
                    .apply_update(Update::decode_v1(&update)?)?;

                Ok(Reaction::pass_on(Message::JustHappened(update)))
            }
            Message::Awareness(payload) => Ok(Reaction::pass_on(Message::Awareness(payload))),
            Message::WhoIsHere => Ok(Reaction::pass_on(Message::WhoIsHere)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crdt_spike::{doc_for, everything, insert, read, state_vector};
    use yrs::updates::encoder::Encode;

    use super::*;

    fn ask(room: &Room, message: Message) -> Reaction {
        room.receive(message).expect("the room should accept this")
    }

    #[test]
    fn a_newcomer_is_told_what_the_room_already_knows() {
        let room = Room::new();
        let author = doc_for(1);
        insert(&author, 0, "The loom stood silent.");
        ask(&room, Message::JustHappened(everything(&author)));

        let Message::WhatDoYouHave(greeting) = room.greet() else {
            panic!("a greeting should be a state vector request");
        };
        let newcomer = doc_for(2);
        let Reaction {
            to_sender: Some(Message::HereIsWhatYouMissed(catch_up)),
            ..
        } = ask(
            &room,
            Message::WhatDoYouHave(state_vector(&newcomer).encode_v1()),
        )
        else {
            panic!("asking what the room has should be answered with an update");
        };
        crdt_spike::absorb(&newcomer, &catch_up);

        assert!(
            !greeting.is_empty(),
            "the room should publish its own state"
        );
        assert_eq!(read(&newcomer), "The loom stood silent.");
    }

    #[test]
    fn an_edit_is_passed_to_the_other_peers_but_not_echoed_back() {
        let room = Room::new();
        let author = doc_for(1);
        insert(&author, 0, "warp");

        let reaction = ask(&room, Message::JustHappened(everything(&author)));

        assert_eq!(
            reaction.to_sender, None,
            "a peer must not receive its own edit"
        );
        assert!(matches!(reaction.to_others, Some(Message::JustHappened(_))));
    }

    #[test]
    fn two_peers_converge_through_the_room() {
        let room = Room::new();
        let ada = doc_for(1);
        let bo = doc_for(2);
        insert(&ada, 0, "ac");

        ask(&room, Message::JustHappened(everything(&ada)));
        let Reaction {
            to_sender: Some(Message::HereIsWhatYouMissed(catch_up)),
            ..
        } = ask(&room, Message::WhatDoYouHave(state_vector(&bo).encode_v1()))
        else {
            panic!("bo should be caught up");
        };
        crdt_spike::absorb(&bo, &catch_up);

        insert(&ada, 1, "b");
        insert(&bo, 1, "B");
        let from_ada = ask(&room, Message::JustHappened(everything(&ada)));
        let from_bo = ask(&room, Message::JustHappened(everything(&bo)));

        let Some(Message::JustHappened(for_bo)) = from_ada.to_others else {
            panic!("ada's edit should travel");
        };
        let Some(Message::JustHappened(for_ada)) = from_bo.to_others else {
            panic!("bo's edit should travel");
        };
        crdt_spike::absorb(&bo, &for_bo);
        crdt_spike::absorb(&ada, &for_ada);

        assert_eq!(read(&ada), read(&bo), "peers must agree");
        assert_eq!(read(&ada).len(), 4, "both inserts should survive");
    }

    #[test]
    fn the_room_holds_the_document_itself_not_just_a_pipe() {
        let room = Room::new();
        let author = doc_for(1);
        insert(&author, 0, "weft");
        ask(&room, Message::JustHappened(everything(&author)));

        let latecomer = doc_for(9);
        let Reaction {
            to_sender: Some(Message::HereIsWhatYouMissed(catch_up)),
            ..
        } = ask(
            &room,
            Message::WhatDoYouHave(state_vector(&latecomer).encode_v1()),
        )
        else {
            panic!("a latecomer should be caught up");
        };
        crdt_spike::absorb(&latecomer, &catch_up);

        assert_eq!(
            read(&latecomer),
            "weft",
            "the room must serve peers who were never connected when the edit happened"
        );
    }

    #[test]
    fn awareness_is_relayed_without_being_understood() {
        let room = Room::new();
        let nonsense = vec![200, 13, 42, 7];

        let reaction = ask(&room, Message::Awareness(nonsense.clone()));

        assert_eq!(reaction.to_others, Some(Message::Awareness(nonsense)));
        assert_eq!(
            everything(room.doc()),
            everything(&Doc::new()),
            "awareness must never touch the document"
        );
    }

    #[test]
    fn asking_who_is_here_reaches_the_other_peers() {
        let room = Room::new();

        let reaction = ask(&room, Message::WhoIsHere);

        assert_eq!(reaction.to_others, Some(Message::WhoIsHere));
    }

    #[test]
    fn a_corrupt_update_is_refused_rather_than_applied() {
        let room = Room::new();

        let outcome = room.receive(Message::JustHappened(vec![255, 255, 255, 255]));

        assert!(outcome.is_err(), "garbage must not be accepted");
    }
}
