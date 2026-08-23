use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use passages_core::{Passage, PassageError, PassageId, PassageService, PassageServiceError};
use thiserror::Error;
use tokio::sync::broadcast;

use crate::protocol::Message;

pub type PeerId = usize;

type Open = HashMap<PassageId, Arc<LivePassage>>;

const BACKLOG: usize = 256;

#[derive(Debug, Error)]
pub enum LivePassageError {
    #[error("a peer sent a frame we could not read: {0}")]
    Frame(#[from] yrs::encoding::read::Error),

    #[error(transparent)]
    Passage(#[from] PassageError),
}

#[derive(Debug, Clone)]
pub struct Overheard {
    pub from: PeerId,
    pub frame: Vec<u8>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    pub to_sender: Option<Message>,
    pub to_others: Option<Message>,
    pub to_store: Option<Vec<u8>>,
}

pub struct LivePassage {
    passage: Passage,
    traffic: broadcast::Sender<Overheard>,
}

impl LivePassage {
    fn open(passage: Passage) -> Self {
        LivePassage {
            passage,
            traffic: broadcast::channel(BACKLOG).0,
        }
    }

    pub fn id(&self) -> PassageId {
        self.passage.id()
    }

    pub fn text(&self) -> String {
        self.passage.text()
    }

    pub fn listen(&self) -> broadcast::Receiver<Overheard> {
        self.traffic.subscribe()
    }

    pub fn broadcast(&self, overheard: Overheard) {
        let _ = self.traffic.send(overheard);
    }

    pub fn greet(&self) -> Reaction {
        Reaction {
            to_sender: Some(Message::WhatDoYouHave(self.passage.state_vector())),
            to_others: Some(Message::WhoIsHere),
            ..Reaction::default()
        }
    }

    pub fn react_to(&self, frame: &[u8]) -> Result<Reaction, LivePassageError> {
        self.receive(Message::decode(frame)?)
    }

    pub fn receive(&self, message: Message) -> Result<Reaction, LivePassageError> {
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

#[derive(Clone)]
pub struct LivePassages {
    open: Arc<RwLock<Open>>,
    peers: Arc<AtomicUsize>,
    service: PassageService,
}

impl LivePassages {
    pub fn new(service: PassageService) -> Self {
        LivePassages {
            open: Arc::new(RwLock::new(Open::new())),
            peers: Arc::new(AtomicUsize::new(1)),
            service,
        }
    }

    pub async fn join(&self, id: PassageId) -> Result<Arc<LivePassage>, PassageServiceError> {
        if let Some(found) = self.find(id) {
            return Ok(found);
        }

        let passage = self.service.open(&id.to_string()).await?;

        Ok(self.keep(id, passage))
    }

    pub async fn persist(&self, id: PassageId, update: &[u8]) -> Result<(), PassageServiceError> {
        self.service.absorb(&id.to_string(), update).await
    }

    pub fn next_peer(&self) -> PeerId {
        self.peers.fetch_add(1, Ordering::Relaxed)
    }

    fn find(&self, id: PassageId) -> Option<Arc<LivePassage>> {
        self.open
            .read()
            .expect("live passages lock poisoned")
            .get(&id)
            .cloned()
    }

    fn keep(&self, id: PassageId, passage: Passage) -> Arc<LivePassage> {
        self.open
            .write()
            .expect("live passages lock poisoned")
            .entry(id)
            .or_insert_with(|| Arc::new(LivePassage::open(passage)))
            .clone()
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

    fn a_live_passage() -> LivePassage {
        LivePassage::open(Passage::empty(an_id(1_000)))
    }

    fn deliver(passage: &LivePassage, message: Message) -> Reaction {
        passage
            .receive(message)
            .expect("the passage should accept this")
    }

    #[test]
    fn a_greeting_publishes_what_the_passage_already_has() {
        let passage = a_live_passage();
        deliver(
            &passage,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );

        let Some(Message::WhatDoYouHave(state_vector)) = passage.greet().to_sender else {
            panic!("a greeting should ask the newcomer for their state");
        };

        assert!(
            !state_vector.is_empty(),
            "the passage should publish its own state"
        );
    }

    #[test]
    fn a_greeting_also_asks_the_others_to_republish_their_cursors() {
        let passage = a_live_passage();

        let reaction = passage.greet();

        assert_eq!(
            reaction.to_others,
            Some(Message::WhoIsHere),
            "without this a newcomer sees no cursors until someone moves"
        );
        assert_eq!(reaction.to_store, None, "joining is not a write");
    }

    #[test]
    fn a_newcomer_asking_what_we_have_is_told_everything() {
        let passage = a_live_passage();
        deliver(
            &passage,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );
        let newcomer = Passage::empty(an_id(2_000));

        let reaction = deliver(&passage, Message::WhatDoYouHave(newcomer.state_vector()));

        let Some(Message::HereIsWhatYouMissed(catch_up)) = reaction.to_sender else {
            panic!("expected a catch-up update, got {:?}", reaction.to_sender);
        };
        newcomer.absorb(&catch_up).expect("catch-up should apply");
        assert_eq!(newcomer.text(), "The loom stood silent.");
    }

    #[test]
    fn catching_a_newcomer_up_tells_nobody_else_and_stores_nothing() {
        let passage = a_live_passage();

        let reaction = deliver(
            &passage,
            Message::WhatDoYouHave(Passage::empty(an_id(2)).state_vector()),
        );

        assert_eq!(reaction.to_others, None);
        assert_eq!(reaction.to_store, None, "a read must not write");
    }

    #[test]
    fn an_edit_reaches_the_other_peers_and_the_store_but_is_not_echoed_back() {
        let passage = a_live_passage();
        let update = a_paragraph("The loom stood silent.");

        let reaction = deliver(&passage, Message::JustHappened(update.clone()));

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
    fn a_live_passage_holds_the_document_itself_not_just_a_pipe() {
        let passage = a_live_passage();

        deliver(
            &passage,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );

        assert_eq!(
            passage.text(),
            "The loom stood silent.",
            "the room must be able to serve a peer who was never connected"
        );
    }

    #[test]
    fn a_catch_up_from_a_peer_is_absorbed_like_any_other_edit() {
        let passage = a_live_passage();

        let reaction = deliver(
            &passage,
            Message::HereIsWhatYouMissed(a_paragraph("The loom stood silent.")),
        );

        assert_eq!(passage.text(), "The loom stood silent.");
        assert!(reaction.to_store.is_some(), "it is still durable prose");
    }

    #[test]
    fn awareness_is_relayed_without_being_understood_or_stored() {
        let passage = a_live_passage();
        let nonsense = vec![200, 13, 42, 7];

        let reaction = deliver(&passage, Message::Awareness(nonsense.clone()));

        assert_eq!(reaction.to_others, Some(Message::Awareness(nonsense)));
        assert_eq!(reaction.to_sender, None);
        assert_eq!(
            reaction.to_store, None,
            "presence must never reach the store"
        );
        assert_eq!(passage.text(), "", "awareness must not touch the document");
    }

    #[test]
    fn asking_who_is_here_reaches_the_other_peers() {
        let passage = a_live_passage();

        let reaction = deliver(&passage, Message::WhoIsHere);

        assert_eq!(reaction.to_others, Some(Message::WhoIsHere));
        assert_eq!(reaction.to_store, None);
    }

    #[test]
    fn an_unusable_update_is_refused_and_changes_nothing() {
        let passage = a_live_passage();
        deliver(
            &passage,
            Message::JustHappened(a_paragraph("The loom stood silent.")),
        );

        let outcome = passage.receive(Message::JustHappened(vec![255, 255, 255, 255]));

        assert!(outcome.is_err(), "garbage must not be accepted");
        assert_eq!(
            passage.text(),
            "The loom stood silent.",
            "a refused update must leave the passage as it was"
        );
    }

    #[test]
    fn a_corrupt_state_vector_is_refused() {
        let passage = a_live_passage();

        assert!(
            passage
                .receive(Message::WhatDoYouHave(vec![255, 255, 255, 255]))
                .is_err()
        );
    }
}
