use std::fmt::{self, Debug, Formatter};

use thiserror::Error;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

use crate::projection::plain_text;
use crate::{IdeaLink, ProjectLink, SceneId, SceneTitle};

#[derive(Debug, Error)]
pub enum SceneError {
    #[error("the payload could not be read: {0}")]
    Unreadable(#[from] yrs::encoding::read::Error),

    #[error("the update could not be applied: {0}")]
    Unusable(#[from] yrs::error::UpdateError),
}

pub struct Scene {
    id: SceneId,
    project: ProjectLink,
    title: SceneTitle,
    ideas: Vec<IdeaLink>,
    doc: Doc,
}

impl Scene {
    pub fn empty(id: SceneId, project: ProjectLink) -> Self {
        Scene {
            id,
            project,
            title: SceneTitle::untitled(),
            ideas: Vec::new(),
            doc: Doc::new(),
        }
    }

    pub fn titled(self, title: SceneTitle) -> Self {
        Self { title, ..self }
    }

    pub fn linked_to(self, ideas: Vec<IdeaLink>) -> Self {
        Self { ideas, ..self }
    }

    pub fn project(&self) -> &ProjectLink {
        &self.project
    }

    pub fn title(&self) -> &SceneTitle {
        &self.title
    }

    pub fn ideas(&self) -> &[IdeaLink] {
        &self.ideas
    }

    pub fn rehydrate(id: SceneId, project: ProjectLink, stored: &[u8]) -> Result<Self, SceneError> {
        let scene = Scene::empty(id, project);
        scene.apply(stored)?;

        Ok(scene)
    }

    pub fn id(&self) -> SceneId {
        self.id
    }

    pub fn readable(update: &[u8]) -> Result<(), SceneError> {
        Doc::new()
            .transact_mut()
            .apply_update(Update::decode_v1(update)?)?;

        Ok(())
    }

    pub fn apply(&self, update: &[u8]) -> Result<(), SceneError> {
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

    pub fn changes_since(&self, state_vector: &[u8]) -> Result<Vec<u8>, SceneError> {
        let theirs = StateVector::decode_v1(state_vector)?;

        Ok(self.doc.transact().encode_state_as_update_v1(&theirs))
    }

    pub fn text(&self) -> String {
        plain_text(&self.doc)
    }
}

impl Debug for Scene {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scene")
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

    fn a_project() -> ProjectLink {
        ProjectLink::from("project_1")
    }

    fn an_id() -> SceneId {
        SceneId::generate(OffsetDateTime::UNIX_EPOCH)
    }

    fn write(scene: &Scene, at: u32, text: &str) {
        let doc = &scene.doc;
        let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, at, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(text));
    }

    fn append(scene: &Scene, at: u32, text: &str) {
        let doc = &scene.doc;
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
    fn a_new_scene_holds_no_prose() {
        let scene = Scene::empty(an_id(), a_project());

        assert_eq!(scene.text(), "");
    }

    #[test]
    fn prose_survives_a_trip_through_storage() {
        let written = Scene::empty(an_id(), a_project());
        write(&written, 0, "The loom stood silent.");

        let reloaded = Scene::rehydrate(an_id(), a_project(), &written.everything())
            .expect("what we stored should reload");

        assert_eq!(reloaded.text(), "The loom stood silent.");
    }

    #[test]
    fn textblocks_are_separated_by_newlines() {
        let scene = Scene::empty(an_id(), a_project());
        write(&scene, 0, "The loom stood silent.");
        write(&scene, 1, "She had not touched it since spring.");

        assert_eq!(
            scene.text(),
            "The loom stood silent.\nShe had not touched it since spring."
        );
    }

    #[test]
    fn two_replicas_editing_apart_converge() {
        let ada = Scene::empty(an_id(), a_project());
        write(&ada, 0, "The loom stood silent.");
        let bo =
            Scene::rehydrate(an_id(), a_project(), &ada.everything()).expect("bo should catch up");

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
        let scene = Scene::empty(an_id(), a_project());
        write(&scene, 0, "The loom stood silent.");
        let update = scene.everything();

        let replica = Scene::empty(an_id(), a_project());
        replica.apply(&update).expect("first apply");
        replica.apply(&update).expect("second apply");

        assert_eq!(replica.text(), "The loom stood silent.");
    }

    #[test]
    fn changes_since_asks_only_for_what_is_missing() {
        let scene = Scene::empty(an_id(), a_project());
        write(&scene, 0, &"a settled paragraph. ".repeat(200));
        let caught_up = scene.state_vector();
        write(&scene, 1, "one late line");

        let catch_up = scene
            .changes_since(&caught_up)
            .expect("our own state vector should be readable");

        assert!(
            catch_up.len() * 10 < scene.everything().len(),
            "catching up cost {} bytes against a {} byte scene",
            catch_up.len(),
            scene.everything().len()
        );
    }

    #[test]
    fn a_fresh_replica_asks_for_everything() {
        let scene = Scene::empty(an_id(), a_project());
        write(&scene, 0, "The loom stood silent.");
        let newcomer = Scene::empty(an_id(), a_project());

        let catch_up = scene
            .changes_since(&newcomer.state_vector())
            .expect("an empty state vector should be readable");

        assert_eq!(catch_up, scene.everything());
    }

    #[test]
    fn a_corrupt_update_is_refused_rather_than_applied() {
        let scene = Scene::empty(an_id(), a_project());
        write(&scene, 0, "The loom stood silent.");

        let outcome = scene.apply(&[255, 255, 255, 255]);

        assert!(outcome.is_err(), "garbage must not be accepted");
        assert_eq!(
            scene.text(),
            "The loom stood silent.",
            "a refused update must leave the scene untouched"
        );
    }

    #[test]
    fn a_corrupt_state_vector_is_refused() {
        let scene = Scene::empty(an_id(), a_project());

        assert!(scene.changes_since(&[255, 255, 255, 255]).is_err());
    }
}
