use std::fmt::{self, Debug, Formatter};

use thiserror::Error;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

use crate::PassageId;
use crate::projection::plain_text;

#[derive(Debug, Error)]
pub enum PassageError {
    #[error("the payload could not be read: {0}")]
    Unreadable(#[from] yrs::encoding::read::Error),

    #[error("the update could not be applied: {0}")]
    Unusable(#[from] yrs::error::UpdateError),
}

pub struct Passage {
    id: PassageId,
    doc: Doc,
}

impl Passage {
    pub fn empty(id: PassageId) -> Self {
        Passage {
            id,
            doc: Doc::new(),
        }
    }

    pub fn rehydrate(id: PassageId, stored: &[u8]) -> Result<Self, PassageError> {
        let passage = Passage::empty(id);
        passage.apply(stored)?;

        Ok(passage)
    }

    pub fn id(&self) -> PassageId {
        self.id
    }

    pub fn apply(&self, update: &[u8]) -> Result<(), PassageError> {
        self.doc
            .transact_mut()
            .apply_update(Update::decode_v1(update)?)?;

        Ok(())
    }

    pub fn everything(&self) -> Vec<u8> {
        self.doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    pub fn state_vector(&self) -> Vec<u8> {
        self.doc.transact().state_vector().encode_v1()
    }

    pub fn changes_since(&self, state_vector: &[u8]) -> Result<Vec<u8>, PassageError> {
        let theirs = StateVector::decode_v1(state_vector)?;

        Ok(self.doc.transact().encode_state_as_update_v1(&theirs))
    }

    pub fn text(&self) -> String {
        plain_text(&self.doc)
    }
}

impl Debug for Passage {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Passage")
            .field("id", &self.id)
            .field("text", &self.text())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use yrs::{Text, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

    use super::*;
    use crate::projection::FRAGMENT;

    fn an_id() -> PassageId {
        PassageId::generate(OffsetDateTime::UNIX_EPOCH)
    }

    fn write(passage: &Passage, at: u32, text: &str) {
        let doc = &passage.doc;
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, at, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(text));
    }

    fn append(passage: &Passage, at: u32, text: &str) {
        let doc = &passage.doc;
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let paragraph = match fragment.get(&txn, at) {
            Some(yrs::types::xml::XmlOut::Element(element)) => element,
            other => panic!("expected a paragraph, found {other:?}"),
        };
        let prose = match paragraph.get(&txn, 0) {
            Some(yrs::types::xml::XmlOut::Text(prose)) => prose,
            other => panic!("expected prose, found {other:?}"),
        };
        let end = prose.len(&txn);
        prose.insert(&mut txn, end, text);
    }

    #[test]
    fn a_new_passage_holds_no_prose() {
        let passage = Passage::empty(an_id());

        assert_eq!(passage.text(), "");
    }

    #[test]
    fn prose_survives_a_trip_through_storage() {
        let written = Passage::empty(an_id());
        write(&written, 0, "The loom stood silent.");

        let reloaded = Passage::rehydrate(an_id(), &written.everything())
            .expect("what we stored should reload");

        assert_eq!(reloaded.text(), "The loom stood silent.");
    }

    #[test]
    fn textblocks_are_separated_by_newlines() {
        let passage = Passage::empty(an_id());
        write(&passage, 0, "The loom stood silent.");
        write(&passage, 1, "She had not touched it since spring.");

        assert_eq!(
            passage.text(),
            "The loom stood silent.\nShe had not touched it since spring."
        );
    }

    #[test]
    fn two_replicas_editing_apart_converge() {
        let ada = Passage::empty(an_id());
        write(&ada, 0, "The loom stood silent.");
        let bo = Passage::rehydrate(an_id(), &ada.everything()).expect("bo should catch up");

        append(&ada, 0, " Ada wrote this.");
        append(&bo, 0, " Bo wrote this.");
        ada.apply(&bo.everything()).expect("bo's edit should apply");
        bo.apply(&ada.everything())
            .expect("ada's edit should apply");

        assert_eq!(ada.text(), bo.text(), "replicas must agree");
        assert!(ada.text().contains("Ada wrote this."));
        assert!(ada.text().contains("Bo wrote this."));
    }

    #[test]
    fn applying_the_same_update_twice_changes_nothing() {
        let passage = Passage::empty(an_id());
        write(&passage, 0, "The loom stood silent.");
        let update = passage.everything();

        let replica = Passage::empty(an_id());
        replica.apply(&update).expect("first apply");
        replica.apply(&update).expect("second apply");

        assert_eq!(replica.text(), "The loom stood silent.");
    }

    #[test]
    fn changes_since_asks_only_for_what_is_missing() {
        let passage = Passage::empty(an_id());
        write(&passage, 0, &"a settled paragraph. ".repeat(200));
        let caught_up = passage.state_vector();
        write(&passage, 1, "one late line");

        let catch_up = passage
            .changes_since(&caught_up)
            .expect("our own state vector should be readable");

        assert!(
            catch_up.len() * 10 < passage.everything().len(),
            "catching up cost {} bytes against a {} byte passage",
            catch_up.len(),
            passage.everything().len()
        );
    }

    #[test]
    fn a_fresh_replica_asks_for_everything() {
        let passage = Passage::empty(an_id());
        write(&passage, 0, "The loom stood silent.");
        let newcomer = Passage::empty(an_id());

        let catch_up = passage
            .changes_since(&newcomer.state_vector())
            .expect("an empty state vector should be readable");

        assert_eq!(catch_up, passage.everything());
    }

    #[test]
    fn a_corrupt_update_is_refused_rather_than_applied() {
        let passage = Passage::empty(an_id());
        write(&passage, 0, "The loom stood silent.");

        let outcome = passage.apply(&[255, 255, 255, 255]);

        assert!(outcome.is_err(), "garbage must not be accepted");
        assert_eq!(
            passage.text(),
            "The loom stood silent.",
            "a refused update must leave the passage untouched"
        );
    }

    #[test]
    fn a_corrupt_state_vector_is_refused() {
        let passage = Passage::empty(an_id());

        assert!(passage.changes_since(&[255, 255, 255, 255]).is_err());
    }
}
