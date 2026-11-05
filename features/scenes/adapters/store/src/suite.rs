#![cfg_attr(
    not(any(feature = "postgres", feature = "sqlite")),
    allow(dead_code, unused_imports, unused_macros)
)]

use std::ops::Deref;

use messaging::{Message, RoutingKey};
use scenes_core::{
    FRAGMENT, IdeaLink, ProjectLink, Scene, SceneChange, SceneId, SceneStore, SceneTitle,
    StoreError,
};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};
use transacting::{Transaction, Transactions};

use crate::compaction::COMPACT_AFTER;
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

#[async_trait::async_trait]
pub trait Workbench: Sized {
    type Store: SceneStore;

    async fn setup() -> Self;

    fn store(&self) -> &Self::Store;

    fn transactions(&self) -> &dyn Transactions;

    fn committing(&self) -> Committing<'_, Self::Store> {
        self.committing_to(self.store())
    }

    fn committing_to<'a>(&'a self, store: &'a Self::Store) -> Committing<'a, Self::Store> {
        Committing {
            store,
            transactions: self.transactions(),
        }
    }

    async fn enqueued(&self) -> Vec<Message>;

    async fn cleanup(self);
}

#[async_trait::async_trait]
pub trait Tables: Workbench {
    fn on_the_same_tables(&self, compacting_after: i64) -> Self::Store;

    async fn compact(&self, id: SceneId) -> Result<bool, StoreError>;

    async fn rows(&self) -> Vec<bool>;

    async fn count(&self, query: &'static str) -> i64;
}

pub struct Committing<'a, S> {
    store: &'a S,
    transactions: &'a dyn Transactions,
}

fn unreachable(failure: transacting::Failure) -> StoreError {
    StoreError::Backend(Box::new(failure))
}

impl<S: SceneStore> Committing<'_, S> {
    pub async fn begun(&self) -> Result<Transaction, StoreError> {
        self.transactions.begin().await.map_err(unreachable)
    }

    pub fn uncommitted(&self) -> &S {
        self.store
    }

    pub async fn create(&self, scene: &Scene) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.create(&mut transaction, scene).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn apply(&self, id: SceneId, update: &[u8]) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.apply(&mut transaction, id, update).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn retitle(&self, id: SceneId, title: &SceneTitle) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.retitle(&mut transaction, id, title).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn announce_word_count(&self, id: SceneId, words: usize) -> Result<(), StoreError> {
        let as_of = self.store.prose_version_of(id).await?;
        self.announce_word_count_as_of(id, words, as_of).await
    }

    pub async fn announce_word_count_as_of(
        &self,
        id: SceneId,
        words: usize,
        as_of: u64,
    ) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store
            .announce_word_count(&mut transaction, id, words, as_of)
            .await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn announce_opening(&self, id: SceneId, opening: &str) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store
            .announce_opening(&mut transaction, id, opening)
            .await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn link(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.link(&mut transaction, id, idea).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn unlink(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.unlink(&mut transaction, id, idea).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.unlink_everywhere(&mut transaction, idea).await?;
        transaction.finish().await.map_err(unreachable)
    }

    pub async fn delete(&self, id: SceneId) -> Result<(), StoreError> {
        let mut transaction = self.begun().await?;
        self.store.delete(&mut transaction, id).await?;
        transaction.finish().await.map_err(unreachable)
    }
}

impl<S> Deref for Committing<'_, S> {
    type Target = S;

    fn deref(&self) -> &S {
        self.store
    }
}

pub fn message_for(change: &SceneChange, at: OffsetDateTime) -> Message {
    let (routing, payload) = match change {
        SceneChange::IdeaLinked {
            scene,
            project,
            idea,
            version,
        } => (
            "scene.idea.linked",
            json!({
                "scene": scene.to_string(),
                "project": project.to_string(),
                "idea": idea.to_string(),
                "version": version,
            }),
        ),
        SceneChange::IdeaUnlinked {
            scene,
            project,
            idea,
            version,
        } => (
            "scene.idea.unlinked",
            json!({
                "scene": scene.to_string(),
                "project": project.to_string(),
                "idea": idea.to_string(),
                "version": version,
            }),
        ),
        SceneChange::Deleted { scene, project } => (
            "scene.deleted",
            json!({ "scene": scene.to_string(), "project": project.to_string() }),
        ),
        SceneChange::Created { scene, project } => (
            "scene.created",
            json!({ "scene": scene.to_string(), "project": project.to_string() }),
        ),
        SceneChange::Retitled {
            scene,
            project,
            title,
            version,
        } => (
            "scene.retitled",
            json!({
                "scene": scene.to_string(),
                "project": project.to_string(),
                "title": title.to_string(),
                "version": version
            }),
        ),
        SceneChange::OpeningChanged {
            scene,
            project,
            opening,
        } => (
            "scene.opening.changed",
            json!({ "scene": scene.to_string(), "project": project.to_string(), "opening": opening }),
        ),
        SceneChange::WordsCounted {
            scene,
            project,
            words,
        } => (
            "scene.words.counted",
            json!({ "scene": scene.to_string(), "project": project.to_string(), "words": words }),
        ),
    };

    Message::opening(
        RoutingKey::parse(routing).expect("a plain key is fine"),
        payload,
        at,
    )
}

fn told_after_creating(messages: Vec<Message>) -> Vec<(String, Value)> {
    told(messages)
        .into_iter()
        .filter(|(routing, _)| routing != "scene.created")
        .collect()
}

fn told(messages: Vec<Message>) -> Vec<(String, Value)> {
    messages
        .into_iter()
        .map(|message| (message.routing.to_string(), message.payload))
        .collect()
}

pub fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

pub fn an_id(seconds: i64) -> SceneId {
    SceneId::generate(at(seconds))
}

pub fn a_paragraph(saying: &str) -> Vec<u8> {
    let doc = Doc::new();
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    {
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(saying));
    }

    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

pub fn a_project() -> ProjectLink {
    ProjectLink::from("project_1")
}

pub fn a_scene(id: SceneId, saying: &str) -> Scene {
    let scene = Scene::empty(id, a_project());
    scene
        .apply(&a_paragraph(saying))
        .expect("sample prose should apply");

    scene
}

pub async fn create_then_load_returns_the_prose(store: &Committing<'_, impl SceneStore>) {
    // given: a scene with prose
    let id = an_id(1_000);

    // when: it is created and loaded
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    let found = store.load(id).await.expect("load should find the scene");

    // then: its prose comes back
    assert_eq!(found.text(), "The loom stood silent.");
    // and: so does its id
    assert_eq!(found.id(), id);
}

pub async fn an_empty_scene_can_be_stored(store: &Committing<'_, impl SceneStore>) {
    // given: an empty scene
    let id = an_id(1_000);

    // when: it is created
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    // then: it loads without prose
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "");
}

pub async fn load_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: it is loaded
    let error = store
        .load(missing)
        .await
        .expect_err("load should not find the scene");

    // then: it is not found
    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn create_rejects_a_duplicate_id(store: &Committing<'_, impl SceneStore>) {
    // given: a stored scene
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("first create should succeed");

    // when: another scene of the same id is created
    let error = store
        .create(&a_scene(id, "Something else entirely."))
        .await
        .expect_err("second create should conflict");

    // then: it conflicts
    assert!(
        matches!(&error, StoreError::Conflict(existing) if *existing == id),
        "expected Conflict({id}), got {error:?}"
    );
}

pub async fn a_rejected_create_leaves_the_stored_scene_untouched(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a stored scene
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("first create should succeed");

    // when: another scene of the same id is created
    let _ = store.create(&a_scene(id, "Something else entirely.")).await;

    // then: the stored scene keeps its prose
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorb_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: an update to it is applied
    let error = store
        .apply(missing, &a_paragraph("into the void"))
        .await
        .expect_err("apply should not find the scene");

    // then: it is not found
    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn absorbed_updates_are_visible_on_load(store: &Committing<'_, impl SceneStore>) {
    // given: an empty stored scene
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    // when: an update is applied to it
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    // then: it loads with the update's prose
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorbing_the_same_update_twice_changes_nothing(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: an empty stored scene
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");
    // and: an update
    let update = a_paragraph("The loom stood silent.");

    // when: the update is applied twice
    store.apply(id, &update).await.expect("first apply");
    store.apply(id, &update).await.expect("second apply");

    // then: its prose is there once
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.text(),
        "The loom stood silent.",
        "a repeated update must not duplicate the prose"
    );
}

pub async fn updates_absorbed_in_either_order_converge(store: &Committing<'_, impl SceneStore>) {
    // given: two updates
    let ada = a_paragraph("Ada wrote this.");
    let bo = a_paragraph("Bo wrote this.");
    // and: two empty stored scenes
    let one = an_id(1_000);
    let other = an_id(2_000);
    for id in [one, other] {
        store
            .create(&Scene::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    // when: each scene takes both updates in a different order
    store.apply(one, &ada).await.expect("apply ada");
    store.apply(one, &bo).await.expect("apply bo");
    store.apply(other, &bo).await.expect("apply bo");
    store.apply(other, &ada).await.expect("apply ada");

    // then: both scenes hold the same prose
    let first = store.load(one).await.expect("load one").text();
    let second = store.load(other).await.expect("load other").text();
    assert_eq!(first, second, "apply order must not matter");
    // and: it holds both updates
    assert!(first.contains("Ada wrote this."));
    assert!(first.contains("Bo wrote this."));
}

pub async fn an_unusable_update_is_refused_and_changes_nothing(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a stored scene with prose
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: an unusable update is applied
    let error = store
        .apply(id, &[255, 255, 255, 255])
        .await
        .expect_err("garbage must not be accepted");

    // then: it is refused
    assert!(
        matches!(&error, StoreError::Unusable(bad) if *bad == id),
        "expected Unusable({id}), got {error:?}"
    );
    // and: the scene is as it was
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.text(),
        "The loom stood silent.",
        "a refused update must leave the scene as it was"
    );
}

pub async fn delete_removes_the_scene(store: &Committing<'_, impl SceneStore>) {
    // given: a stored scene
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: it is deleted
    store.delete(id).await.expect("delete should succeed");

    // then: it is not found
    let found = store.load(id).await;
    assert!(
        matches!(&found, Err(StoreError::NotFound(gone)) if *gone == id),
        "expected NotFound({id}) after delete, got {found:?}"
    );
}

pub async fn delete_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: it is deleted
    let error = store
        .delete(missing)
        .await
        .expect_err("delete should not find the scene");

    // then: it is not found
    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn a_loaded_scene_still_knows_its_project(store: &Committing<'_, impl SceneStore>) {
    // given: a stored scene
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: it is loaded
    let found = store.load(id).await.expect("load should find the scene");

    // then: it knows its project
    assert_eq!(found.project(), &a_project());
}

pub async fn a_project_lists_its_own_scenes_in_order(store: &Committing<'_, impl SceneStore>) {
    // given: two scenes created out of order
    let first = an_id(1_000);
    let second = an_id(2_000);
    for id in [second, first] {
        store
            .create(&Scene::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    // when: the project's scenes are listed
    let found = store
        .in_project(&a_project(), None, 10)
        .await
        .expect("listing should succeed");

    // then: they come in order
    assert_eq!(found, vec![first, second]);
}

pub async fn another_project_sees_none_of_them(store: &Committing<'_, impl SceneStore>) {
    // given: a scene in one project
    let mine = an_id(1_000);
    store
        .create(&Scene::empty(mine, a_project()))
        .await
        .expect("create should succeed");

    // when: another project's scenes are listed
    let found = store
        .in_project(&ProjectLink::from("project_2"), None, 10)
        .await
        .expect("listing should succeed");

    // then: there are none
    assert!(
        found.is_empty(),
        "a sweep must never reach into another author's project, got {found:?}"
    );
}

pub async fn listing_resumes_after_the_cursor(store: &Committing<'_, impl SceneStore>) {
    // given: three scenes in a project
    let first = an_id(1_000);
    let second = an_id(2_000);
    let third = an_id(3_000);
    for id in [first, second, third] {
        store
            .create(&Scene::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    // when: they are listed two at a time, each batch after the last
    let batch = store
        .in_project(&a_project(), None, 2)
        .await
        .expect("first batch should succeed");
    let rest = store
        .in_project(&a_project(), batch.last().copied(), 2)
        .await
        .expect("second batch should succeed");

    // then: the first batch holds the first two
    assert_eq!(batch, vec![first, second]);
    // and: the next one the rest
    assert_eq!(
        rest,
        vec![third],
        "without an exclusive cursor a sweep hands itself the same batch forever"
    );
}

pub async fn listing_past_the_last_scene_is_empty(store: &Committing<'_, impl SceneStore>) {
    // given: a project of one scene
    let only = an_id(1_000);
    store
        .create(&Scene::empty(only, a_project()))
        .await
        .expect("create should succeed");

    // when: its scenes after that one are listed
    let found = store
        .in_project(&a_project(), Some(only), 10)
        .await
        .expect("listing should succeed");

    // then: there are none
    assert!(found.is_empty(), "expected nothing after the last id");
}

pub async fn a_deleted_scene_leaves_the_listing(store: &Committing<'_, impl SceneStore>) {
    // given: a stored scene
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    // when: it is deleted
    store.delete(id).await.expect("delete should succeed");

    // then: its project lists nothing
    let found = store
        .in_project(&a_project(), None, 10)
        .await
        .expect("listing should succeed");
    assert!(found.is_empty());
}

fn a_title(saying: &str) -> SceneTitle {
    SceneTitle::new(saying).expect("a plain title is fine")
}

pub async fn a_new_scene_starts_untitled(store: &Committing<'_, impl SceneStore>) {
    // given: a new stored scene
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    // when: it is loaded
    let found = store.load(id).await.expect("load should find the scene");

    // then: it is untitled
    assert!(found.title().is_untitled());
}

pub async fn a_retitled_scene_keeps_its_title(store: &Committing<'_, impl SceneStore>) {
    // given: a stored scene
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    // when: it is retitled
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    // then: it loads with its title
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.title(), &a_title("The loom"));
}

pub async fn retitling_a_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: it is retitled
    let error = store
        .retitle(missing, &a_title("Nowhere"))
        .await
        .expect_err("retitle should not find the scene");

    // then: it is not found
    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn retitling_leaves_the_text_as_it_was(store: &Committing<'_, impl SceneStore>) {
    // given: a stored scene with prose
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: it is retitled
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    // then: its prose is as it was
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn a_title_survives_further_writing(store: &Committing<'_, impl SceneStore>) {
    // given: a titled scene
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    // when: more is written in it
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    // then: it keeps its title
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.title(),
        &a_title("The loom"),
        "writing rewrites the stored scene, and the title must ride along rather than \
         reset to untitled with every keystroke"
    );
}

pub async fn announcing_a_word_count_for_a_missing_scene_is_not_found(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: its word count is announced
    let error = store
        .announce_word_count(missing, 42)
        .await
        .expect_err("announcing should not find the scene");

    // then: it is not found
    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

async fn uncounted(store: &Committing<'_, impl SceneStore>) -> Vec<SceneId> {
    store
        .uncounted(10)
        .await
        .expect("asking what is uncounted should succeed")
}

pub async fn writing_leaves_a_scene_owing_its_count_until_the_count_is_told(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: two empty scenes
    let (written, quiet) = (an_id(1_000), an_id(2_000));
    an_empty_scene(store, written).await;
    an_empty_scene(store, quiet).await;
    // and: neither owes a count
    assert!(
        uncounted(store).await.is_empty(),
        "a scene only made owes no count"
    );

    // when: one is written in
    store
        .apply(written, &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    // then: it alone owes a count
    assert_eq!(
        uncounted(store).await,
        vec![written],
        "the prose and the count it owes are kept together, so a crash cannot lose the debt"
    );

    // when: its count is told
    store
        .announce_word_count(written, 4)
        .await
        .expect("announcing should succeed");
    // then: none owes a count
    assert!(uncounted(store).await.is_empty());
}

pub async fn a_count_as_of_an_older_version_leaves_the_newer_writing_owed(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene written in
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    let counted_as_of = store
        .prose_version_of(id)
        .await
        .expect("the prose has a version");
    // and: written in again since
    store
        .apply(id, &a_paragraph("Then it spoke."))
        .await
        .expect("writing should succeed");

    // when: a count as of the first writing is told
    store
        .announce_word_count_as_of(id, 4, counted_as_of)
        .await
        .expect("announcing should succeed");

    // then: it still owes a count
    assert_eq!(
        uncounted(store).await,
        vec![id],
        "what was written after the words were read is not in the count, so it is still owed"
    );

    // when: a count as of now is told
    store
        .announce_word_count(id, 7)
        .await
        .expect("announcing should succeed");
    // then: it owes none
    assert!(uncounted(store).await.is_empty());
}

pub async fn the_version_of_a_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: its version is asked for
    let error = store
        .version_of(missing)
        .await
        .expect_err("there is no version of what is not there");

    // then: it is not found
    assert!(matches!(&error, StoreError::NotFound(id) if *id == missing));
}

pub async fn a_transaction_is_told_the_version_it_moved_the_scene_to(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene written in once
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");

    // when: a transaction retitles it and asks for its version
    let mut transaction = store.begun().await.expect("beginning should succeed");
    let before = store
        .uncommitted()
        .version_within(&mut transaction, id)
        .await
        .expect("the scene has a version");
    store
        .uncommitted()
        .retitle(&mut transaction, id, &a_title("The loom"))
        .await
        .expect("retitling should succeed");
    let after = store
        .uncommitted()
        .version_within(&mut transaction, id)
        .await
        .expect("the scene has a version");
    transaction
        .finish()
        .await
        .expect("committing should succeed");

    // then: it was told the version the writing left untouched, and the one its own retitling moved it to
    assert_eq!(
        (before, after),
        (0, 1),
        "the prose counts its own versions, so writing does not move the scene's"
    );
    // and: that is the version the scene is at
    assert_eq!(
        store.version_of(id).await.expect("the scene has a version"),
        1
    );
}

pub async fn the_prose_version_of_a_missing_scene_is_not_found(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: the version of its prose is asked for
    let error = store
        .prose_version_of(missing)
        .await
        .expect_err("there is no version of what is not there");

    // then: it is not found
    assert!(matches!(&error, StoreError::NotFound(id) if *id == missing));
}

pub async fn the_prose_and_the_scene_count_their_versions_apart(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: an empty scene
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: it is written in twice
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    store
        .apply(id, &a_paragraph("Then it spoke."))
        .await
        .expect("writing should succeed");
    // then: the prose moved twice and the scene not at all
    assert_eq!(
        (
            store
                .prose_version_of(id)
                .await
                .expect("the prose has a version"),
            store.version_of(id).await.expect("the scene has a version"),
        ),
        (2, 0),
        "typing must not make a retitle read before it look outdated"
    );

    // when: it is retitled, linked and unlinked
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitling should succeed");
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("linking should succeed");
    store
        .unlink(id, &an_idea("idea_1"))
        .await
        .expect("unlinking should succeed");
    // then: the scene moved three times and the prose not at all
    assert_eq!(
        (
            store
                .prose_version_of(id)
                .await
                .expect("the prose has a version"),
            store.version_of(id).await.expect("the scene has a version"),
        ),
        (2, 3),
        "what the scene's routes change leaves the count of the prose owed or not as it was"
    );
}

pub async fn the_version_within_a_transaction_of_a_missing_scene_is_not_found(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: a transaction asks for its version
    let mut transaction = store.begun().await.expect("beginning should succeed");
    let error = store
        .uncommitted()
        .version_within(&mut transaction, missing)
        .await
        .expect_err("there is no version of what is not there");

    // then: it is not found
    assert!(matches!(&error, StoreError::NotFound(id) if *id == missing));
}

pub async fn a_write_rolled_back_owes_no_count(store: &Committing<'_, impl SceneStore>) {
    // given: an empty scene
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: a writing in it is dropped with its transaction
    let mut transaction = store.begun().await.expect("beginning should succeed");
    store
        .uncommitted()
        .apply(&mut transaction, id, &a_paragraph("Never kept."))
        .await
        .expect("writing should succeed");
    drop(transaction);

    // then: it owes no count
    assert!(uncounted(store).await.is_empty());
}

fn an_idea(named: &str) -> IdeaLink {
    IdeaLink::from(named)
}

async fn an_empty_scene(store: &Committing<'_, impl SceneStore>, id: SceneId) {
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");
}

pub async fn a_new_scene_has_no_linked_ideas(store: &Committing<'_, impl SceneStore>) {
    // given: a new scene
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: it is loaded
    let found = store.load(id).await.expect("load should find the scene");

    // then: it has no linked ideas
    assert!(found.ideas().is_empty());
}

pub async fn linked_ideas_come_back_in_the_order_they_were_linked(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: ideas are linked to it out of their sort order
    for idea in ["idea_c", "idea_a", "idea_b"] {
        store
            .link(id, &an_idea(idea))
            .await
            .expect("link should succeed");
    }

    // then: they come back in the order they were linked
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.ideas(),
        &[an_idea("idea_c"), an_idea("idea_a"), an_idea("idea_b")],
        "the author reads this list as the order they reached for the ideas, not the order \
         the ids happen to sort in"
    );
}

pub async fn linking_the_same_idea_twice_is_one_link(store: &Committing<'_, impl SceneStore>) {
    // given: a scene
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: the same idea is linked to it twice
    for _ in 0..2 {
        store
            .link(id, &an_idea("idea_1"))
            .await
            .expect("a repeated link is not a failure");
    }

    // then: it is linked once
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.ideas(), &[an_idea("idea_1")]);
}

pub async fn unlinking_takes_away_only_that_idea(store: &Committing<'_, impl SceneStore>) {
    // given: a scene with two linked ideas
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    for idea in ["idea_1", "idea_2"] {
        store
            .link(id, &an_idea(idea))
            .await
            .expect("link should succeed");
    }

    // when: one is unlinked
    store
        .unlink(id, &an_idea("idea_1"))
        .await
        .expect("unlink should succeed");

    // then: only the other is linked
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.ideas(), &[an_idea("idea_2")]);
}

pub async fn unlinking_an_idea_never_linked_is_harmless(store: &Committing<'_, impl SceneStore>) {
    // given: a scene
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // then: unlinking an idea it never linked succeeds
    store
        .unlink(id, &an_idea("idea_elsewhere"))
        .await
        .expect("nothing to unlink is not a failure");
}

pub async fn linking_to_a_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: an idea is linked to it and unlinked from it
    let linked = store.link(missing, &an_idea("idea_1")).await;
    let unlinked = store.unlink(missing, &an_idea("idea_1")).await;

    // then: linking finds no scene
    assert!(
        matches!(&linked, Err(StoreError::NotFound(id)) if *id == missing),
        "expected NotFound({missing}) on link, got {linked:?}"
    );
    // and: neither does unlinking
    assert!(
        matches!(&unlinked, Err(StoreError::NotFound(id)) if *id == missing),
        "expected NotFound({missing}) on unlink, got {unlinked:?}"
    );
}

pub async fn unlinking_an_idea_everywhere_reaches_every_scene(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: two scenes, each linked to the same two ideas
    let one = an_id(1_000);
    let other = an_id(2_000);
    for id in [one, other] {
        an_empty_scene(store, id).await;
        for idea in ["idea_gone", "idea_kept"] {
            store
                .link(id, &an_idea(idea))
                .await
                .expect("link should succeed");
        }
    }

    // when: one idea is unlinked everywhere
    store
        .unlink_everywhere(&an_idea("idea_gone"))
        .await
        .expect("unlinking everywhere should succeed");

    // then: each scene keeps only the other idea
    for id in [one, other] {
        let found = store.load(id).await.expect("load should find the scene");
        assert_eq!(
            found.ideas(),
            &[an_idea("idea_kept")],
            "a discarded idea is unlinked from every scene that linked it, and only that idea is"
        );
    }
}

pub async fn links_survive_further_writing(store: &Committing<'_, impl SceneStore>) {
    // given: a scene with a linked idea
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    // when: more is written in it
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    // then: the idea is still linked
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.ideas(),
        &[an_idea("idea_1")],
        "writing rewrites the stored scene, and the links must ride along"
    );
}

pub async fn linking_leaves_the_title_and_the_text_alone(store: &Committing<'_, impl SceneStore>) {
    // given: a titled scene with prose
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    // when: an idea is linked to it
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    // then: its title is as it was
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.title(), &a_title("The loom"));
    // and: so is its prose
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn linking_an_idea_enqueues_one_message(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: the same idea is linked to it twice
    for _ in 0..2 {
        store
            .link(id, &an_idea("idea_1"))
            .await
            .expect("link should succeed");
    }

    // then: one link is told
    assert_eq!(
        told_after_creating(bench.enqueued().await),
        vec![(
            "scene.idea.linked".to_owned(),
            json!({ "scene": id.to_string(), "project": "project_1", "idea": "idea_1", "version": 1 })
        )],
        "a link that was already there changes nothing, so a listener must not hear it twice"
    );
}

pub async fn unlinking_enqueues_only_for_a_link_that_was_there(bench: &impl Workbench) {
    // given: a scene with a linked idea
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    // when: that idea is unlinked
    store
        .unlink(id, &an_idea("idea_1"))
        .await
        .expect("unlink should succeed");
    // and: an idea never linked is unlinked
    store
        .unlink(id, &an_idea("idea_never"))
        .await
        .expect("unlinking nothing should succeed");

    // then: only the link and its unlinking are told, each with the scene's project
    let heard: Vec<(String, Value)> = told_after_creating(bench.enqueued().await)
        .into_iter()
        .map(|(routing, payload)| (routing, payload["project"].clone()))
        .collect();
    assert_eq!(
        heard,
        vec![
            ("scene.idea.linked".to_owned(), json!("project_1")),
            ("scene.idea.unlinked".to_owned(), json!("project_1")),
        ],
        "each names the scene's project, so a listener can tell where it happened"
    );
}

pub async fn every_change_to_a_scene_s_title_or_links_counts_up(bench: &impl Workbench) {
    // given: two scenes
    let store = &bench.committing();
    let id = an_id(1_000);
    let other = an_id(2_000);
    an_empty_scene(store, id).await;
    an_empty_scene(store, other).await;

    // when: they are linked, unlinked, retitled and written in, some of it changing nothing
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");
    store
        .unlink(id, &an_idea("idea_never"))
        .await
        .expect("unlinking nothing should succeed");
    store
        .link(other, &an_idea("idea_1"))
        .await
        .expect("link should succeed");
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");
    store
        .unlink(id, &an_idea("idea_1"))
        .await
        .expect("unlink should succeed");
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    // then: each scene's announcements carry its count of real changes to its title and links
    let versions: Vec<(String, u64)> = told_after_creating(bench.enqueued().await)
        .into_iter()
        .map(|(_, payload)| {
            (
                payload["scene"].as_str().expect("a scene").to_owned(),
                payload["version"].as_u64().expect("a version"),
            )
        })
        .collect();
    assert_eq!(
        versions,
        vec![
            (id.to_string(), 1),
            (other.to_string(), 1),
            (id.to_string(), 2),
            (id.to_string(), 3),
            (id.to_string(), 4),
        ],
        "a scene's title and links count up, each scene on its own; its prose and what changed nothing do not"
    );
}

pub async fn announcing_a_word_count_enqueues_it_with_the_scene_s_project(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: its word count is announced
    store
        .announce_word_count(id, 42)
        .await
        .expect("announcing should succeed");

    // then: the count is told with the scene's project
    assert_eq!(
        told_after_creating(bench.enqueued().await),
        vec![(
            "scene.words.counted".to_owned(),
            json!({ "scene": id.to_string(), "project": a_project().to_string(), "words": 42 })
        )]
    );
}

pub async fn announcing_a_word_count_is_not_a_change_to_the_scene(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: its word count is announced between two links
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");
    store
        .announce_word_count(id, 42)
        .await
        .expect("announcing should succeed");
    store
        .link(id, &an_idea("idea_2"))
        .await
        .expect("link should succeed");

    // then: the links follow one another in its count of changes
    let versions: Vec<u64> = told(bench.enqueued().await)
        .into_iter()
        .filter(|(routing, _)| routing == "scene.idea.linked")
        .map(|(_, payload)| payload["version"].as_u64().expect("a version"))
        .collect();
    assert_eq!(
        versions,
        vec![1, 2],
        "a count is worked out from the text, so announcing it does not count the scene up"
    );
}

pub async fn deleting_a_scene_enqueues_a_message(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: it is deleted
    store.delete(id).await.expect("delete should succeed");

    // then: its deletion is told with its project
    assert_eq!(
        told_after_creating(bench.enqueued().await),
        vec![(
            "scene.deleted".to_owned(),
            json!({ "scene": id.to_string(), "project": "project_1" })
        )],
        "a scene leaving takes its links with it, and whoever indexed them has to hear so"
    );
}

pub async fn a_refused_change_enqueues_nothing(bench: &impl Workbench) {
    // given: a scene never created
    let store = &bench.committing();
    let missing = an_id(1_000);

    // when: it is linked, unlinked and deleted
    let _ = store.link(missing, &an_idea("idea_1")).await;
    let _ = store.unlink(missing, &an_idea("idea_1")).await;
    let _ = store.delete(missing).await;

    // then: nothing is told
    assert!(
        bench.enqueued().await.is_empty(),
        "an message_for of something that did not happen sends the listener after a ghost"
    );
}

pub async fn writing_enqueues_nothing(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: it is written in
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");
    // and: an idea is unlinked everywhere
    store
        .unlink_everywhere(&an_idea("idea_1"))
        .await
        .expect("unlinking everywhere should succeed");

    // then: nothing is told
    assert!(
        told_after_creating(bench.enqueued().await).is_empty(),
        "every keystroke is an edit, so text is announced by what is counted from it, not edit          by edit; and unlinking a discarded idea everywhere is answered by whoever heard the          discard itself"
    );
}

pub async fn creating_a_scene_is_announced_with_its_project(bench: &impl Workbench) {
    // given: a scene id
    let store = &bench.committing();
    let id = an_id(1_000);

    // when: a scene of it is created
    an_empty_scene(store, id).await;

    // then: its creation is told with its project
    assert_eq!(
        told(bench.enqueued().await),
        vec![(
            "scene.created".to_owned(),
            json!({ "scene": id.to_string(), "project": a_project().to_string() })
        )],
        "a listing kept apart from the store learns of a scene this way"
    );
}

pub async fn a_retitle_is_announced_with_its_title_and_version(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: it is retitled
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    // then: the retitle is told with its title and version
    assert_eq!(
        told_after_creating(bench.enqueued().await),
        vec![(
            "scene.retitled".to_owned(),
            json!({
                "scene": id.to_string(),
                "project": a_project().to_string(),
                "title": "The loom",
                "version": 1
            })
        )],
        "the version tells a late retitle from a newer one"
    );
}

pub async fn announcing_an_opening_enqueues_it_with_the_scene_s_project(bench: &impl Workbench) {
    // given: a scene
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    // when: its opening is announced
    store
        .announce_opening(id, "The loom stood silent.")
        .await
        .expect("announcing should succeed");

    // then: the opening is told with the scene's project
    assert_eq!(
        told_after_creating(bench.enqueued().await),
        vec![(
            "scene.opening.changed".to_owned(),
            json!({
                "scene": id.to_string(),
                "project": a_project().to_string(),
                "opening": "The loom stood silent."
            })
        )]
    );
}

pub async fn announcing_an_opening_for_a_missing_scene_is_not_found(
    store: &Committing<'_, impl SceneStore>,
) {
    // given: a scene never created
    let missing = an_id(1_000);

    // when: its opening is announced
    let error = store
        .announce_opening(missing, "Nowhere.")
        .await
        .expect_err("announcing should not find the scene");

    // then: it is not found
    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone_and_told_to_nobody(
    bench: &impl Workbench,
) {
    // given: a stored scene
    let store = &bench.committing();
    let kept = an_id(1_000);
    store
        .create(&a_scene(kept, "The loom stood silent."))
        .await
        .expect("creating should succeed");
    let told_before = bench.enqueued().await.len();
    // and: a transaction that creates a scene and writes in, retitles and links the stored one
    let mut transaction = store.begun().await.expect("beginning should succeed");
    let uncommitted = store.uncommitted();
    uncommitted
        .create(&mut transaction, &a_scene(an_id(2_000), "Never was."))
        .await
        .expect("creating should succeed");
    uncommitted
        .apply(&mut transaction, kept, &a_paragraph("Never said."))
        .await
        .expect("applying should succeed");
    uncommitted
        .retitle(&mut transaction, kept, &a_title("Never titled"))
        .await
        .expect("retitling should succeed");
    uncommitted
        .link(&mut transaction, kept, &an_idea("idea_1"))
        .await
        .expect("linking should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the stored scene is as it was
    let scene = store.load(kept).await.expect("the kept scene is there");
    assert_eq!(scene.text(), "The loom stood silent.");
    assert!(scene.ideas().is_empty());
    // and: the created one is not there
    assert!(matches!(
        store.load(an_id(2_000)).await,
        Err(StoreError::NotFound(_))
    ));
    // and: nothing more is told
    assert_eq!(
        bench.enqueued().await.len(),
        told_before,
        "nothing written in a dropped transaction is told"
    );
}

macro_rules! conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! bench_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::suite::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::suite::bench_case!(
            $workbench,
            what_was_written_in_a_transaction_that_is_dropped_is_gone_and_told_to_nobody
        );
        $crate::suite::conformance_case!($workbench, create_then_load_returns_the_prose);
        $crate::suite::conformance_case!(
            $workbench,
            writing_leaves_a_scene_owing_its_count_until_the_count_is_told
        );
        $crate::suite::conformance_case!(
            $workbench,
            a_count_as_of_an_older_version_leaves_the_newer_writing_owed
        );
        $crate::suite::conformance_case!($workbench, the_version_of_a_missing_scene_is_not_found);
        $crate::suite::conformance_case!(
            $workbench,
            a_transaction_is_told_the_version_it_moved_the_scene_to
        );
        $crate::suite::conformance_case!(
            $workbench,
            the_version_within_a_transaction_of_a_missing_scene_is_not_found
        );
        $crate::suite::conformance_case!(
            $workbench,
            the_prose_version_of_a_missing_scene_is_not_found
        );
        $crate::suite::conformance_case!(
            $workbench,
            the_prose_and_the_scene_count_their_versions_apart
        );
        $crate::suite::conformance_case!($workbench, a_write_rolled_back_owes_no_count);
        $crate::suite::conformance_case!($workbench, an_empty_scene_can_be_stored);
        $crate::suite::conformance_case!($workbench, load_missing_scene_is_not_found);
        $crate::suite::conformance_case!($workbench, create_rejects_a_duplicate_id);
        $crate::suite::conformance_case!(
            $workbench,
            a_rejected_create_leaves_the_stored_scene_untouched
        );
        $crate::suite::conformance_case!($workbench, absorb_missing_scene_is_not_found);
        $crate::suite::conformance_case!($workbench, absorbed_updates_are_visible_on_load);
        $crate::suite::conformance_case!(
            $workbench,
            absorbing_the_same_update_twice_changes_nothing
        );
        $crate::suite::conformance_case!($workbench, updates_absorbed_in_either_order_converge);
        $crate::suite::conformance_case!(
            $workbench,
            an_unusable_update_is_refused_and_changes_nothing
        );
        $crate::suite::conformance_case!($workbench, delete_removes_the_scene);
        $crate::suite::conformance_case!($workbench, delete_missing_scene_is_not_found);
        $crate::suite::conformance_case!($workbench, a_loaded_scene_still_knows_its_project);
        $crate::suite::conformance_case!($workbench, a_project_lists_its_own_scenes_in_order);
        $crate::suite::conformance_case!($workbench, another_project_sees_none_of_them);
        $crate::suite::conformance_case!($workbench, listing_resumes_after_the_cursor);
        $crate::suite::conformance_case!($workbench, listing_past_the_last_scene_is_empty);
        $crate::suite::conformance_case!($workbench, a_deleted_scene_leaves_the_listing);
        $crate::suite::conformance_case!($workbench, a_new_scene_starts_untitled);
        $crate::suite::conformance_case!($workbench, a_retitled_scene_keeps_its_title);
        $crate::suite::conformance_case!($workbench, retitling_a_missing_scene_is_not_found);
        $crate::suite::conformance_case!($workbench, retitling_leaves_the_text_as_it_was);
        $crate::suite::conformance_case!($workbench, a_title_survives_further_writing);
        $crate::suite::conformance_case!(
            $workbench,
            announcing_a_word_count_for_a_missing_scene_is_not_found
        );
        $crate::suite::conformance_case!(
            $workbench,
            announcing_an_opening_for_a_missing_scene_is_not_found
        );
        $crate::suite::conformance_case!($workbench, a_new_scene_has_no_linked_ideas);
        $crate::suite::conformance_case!(
            $workbench,
            linked_ideas_come_back_in_the_order_they_were_linked
        );
        $crate::suite::conformance_case!($workbench, linking_the_same_idea_twice_is_one_link);
        $crate::suite::conformance_case!($workbench, unlinking_takes_away_only_that_idea);
        $crate::suite::conformance_case!($workbench, unlinking_an_idea_never_linked_is_harmless);
        $crate::suite::conformance_case!($workbench, linking_to_a_missing_scene_is_not_found);
        $crate::suite::conformance_case!(
            $workbench,
            unlinking_an_idea_everywhere_reaches_every_scene
        );
        $crate::suite::conformance_case!($workbench, links_survive_further_writing);
        $crate::suite::conformance_case!($workbench, linking_leaves_the_title_and_the_text_alone);
        $crate::suite::bench_case!($workbench, linking_an_idea_enqueues_one_message);
        $crate::suite::bench_case!(
            $workbench,
            unlinking_enqueues_only_for_a_link_that_was_there
        );
        $crate::suite::bench_case!(
            $workbench,
            every_change_to_a_scene_s_title_or_links_counts_up
        );
        $crate::suite::bench_case!(
            $workbench,
            announcing_a_word_count_enqueues_it_with_the_scene_s_project
        );
        $crate::suite::bench_case!(
            $workbench,
            announcing_a_word_count_is_not_a_change_to_the_scene
        );
        $crate::suite::bench_case!($workbench, deleting_a_scene_enqueues_a_message);
        $crate::suite::bench_case!($workbench, a_refused_change_enqueues_nothing);
        $crate::suite::bench_case!($workbench, writing_enqueues_nothing);
        $crate::suite::bench_case!($workbench, creating_a_scene_is_announced_with_its_project);
        $crate::suite::bench_case!(
            $workbench,
            a_retitle_is_announced_with_its_title_and_version
        );
        $crate::suite::bench_case!(
            $workbench,
            announcing_an_opening_enqueues_it_with_the_scene_s_project
        );
    };
}

pub async fn an_update_is_one_row_and_never_rewrites_the_scene(bench: &impl Tables) {
    // given: a stored scene
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: an update is applied to it
    bench
        .committing()
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    // then: the snapshot stays and the update is one row after it
    assert_eq!(
        bench.rows().await,
        vec![true, false],
        "a snapshot from create, then one appended update — the scene itself is never rewritten"
    );
}

pub async fn a_tail_longer_than_we_keep_is_collapsed_into_a_snapshot(bench: &impl Tables) {
    // given: a store that keeps three updates
    let store_tables = bench.on_the_same_tables(3);
    let store = &bench.committing_to(&store_tables);
    // and: a stored scene
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: five updates are applied to it
    for nth in 0..5 {
        store
            .apply(id, &a_paragraph(&format!("Line {nth}.")))
            .await
            .expect("apply should succeed");
    }

    // then: fewer rows are left than were written
    let parts = bench.rows().await;
    assert!(
        parts.len() < 6,
        "six writes should not have left six rows: {parts:?}"
    );
    // and: they begin with a snapshot
    assert_eq!(
        parts.first(),
        Some(&true),
        "what remains must begin with a snapshot: {parts:?}"
    );
    // and: no prose is lost
    let found = store.load(id).await.expect("load should succeed");
    assert!(
        found.text().contains("The loom stood silent.") && found.text().contains("Line 4."),
        "compaction must not lose prose, got {:?}",
        found.text()
    );
}

pub async fn compacting_a_short_tail_does_nothing(bench: &impl Tables) {
    // given: a scene with one update
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .committing()
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    // when: it is compacted
    let collapsed = bench.compact(id).await.expect("compacting should succeed");

    // then: nothing is collapsed
    assert!(!collapsed, "two rows are not worth collapsing");
    // and: both rows are left
    assert_eq!(bench.rows().await.len(), 2);
}

pub async fn compacting_a_scene_that_was_never_created_is_not_found(bench: &impl Tables) {
    // given: a scene never created
    let never_written = an_id(1_000);

    // when: it is compacted
    let refused = bench.compact(never_written).await;

    // then: it is not found
    assert!(
        matches!(refused, Err(StoreError::NotFound(missing)) if missing == never_written),
        "got {refused:?}"
    );
}

pub async fn an_update_nothing_can_read_writes_no_row(bench: &impl Tables) {
    // given: a stored scene
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: an unreadable update is applied to it
    let refused = bench.committing().apply(id, &[255, 255, 255, 255]).await;

    // then: it is refused
    assert!(
        matches!(refused, Err(StoreError::Unusable(_))),
        "got {refused:?}"
    );
    // and: only the snapshot is there
    assert_eq!(
        bench.rows().await,
        vec![true],
        "garbage must not reach the table, or every load would fail forever after"
    );
}

pub async fn an_update_for_a_scene_that_does_not_exist_writes_no_row(bench: &impl Tables) {
    // when: an update to a scene never created is applied
    let refused = bench
        .committing()
        .apply(an_id(1_000), &a_paragraph("Out of nowhere."))
        .await;

    // then: it is not found
    assert!(
        matches!(refused, Err(StoreError::NotFound(_))),
        "got {refused:?}"
    );
    // and: no row is written
    assert!(bench.rows().await.is_empty());
}

pub async fn deleting_a_scene_takes_its_updates_with_it(bench: &impl Tables) {
    // given: a scene with an update
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .committing()
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    // when: it is deleted
    bench
        .committing()
        .delete(id)
        .await
        .expect("delete should succeed");

    // then: no scene is left
    assert_eq!(bench.count("SELECT count(*) FROM scenes").await, 0);
    // and: no row of it
    assert!(
        bench.rows().await.is_empty(),
        "the parts are the scene — leaving them would leave a scene that load cannot find"
    );
}

pub async fn a_scene_survives_being_reloaded_by_a_store_that_never_saw_the_writes(
    bench: &impl Tables,
) {
    // given: a scene with an update
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .committing()
        .apply(id, &a_paragraph("Then it began."))
        .await
        .expect("apply should succeed");

    // when: another store on the same tables loads it
    let elsewhere_tables = bench.on_the_same_tables(COMPACT_AFTER);
    let elsewhere = &bench.committing_to(&elsewhere_tables);
    let found = elsewhere.load(id).await.expect("load should succeed");

    // then: all its prose comes back
    assert!(
        found.text().contains("The loom stood silent.") && found.text().contains("Then it began."),
        "prose has to come back out of the rows, not out of whoever wrote them, got {:?}",
        found.text()
    );
}

pub async fn deleting_a_scene_takes_its_links_with_it(bench: &impl Tables) {
    // given: a scene with a linked idea
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    bench
        .committing()
        .link(id, &IdeaLink::from("idea_1"))
        .await
        .expect("link should succeed");

    // when: it is deleted
    bench
        .committing()
        .delete(id)
        .await
        .expect("delete should succeed");

    // then: no link is left
    let links = bench.count("SELECT count(*) FROM scene_ideas").await;
    assert_eq!(
        links, 0,
        "a link outliving its scene points at nothing, and nothing sweeps it later"
    );
}

pub async fn a_tail_exactly_as_long_as_we_keep_is_left_as_it_is(bench: &impl Tables) {
    // given: a store that keeps three updates
    let store_tables = bench.on_the_same_tables(3);
    let store = &bench.committing_to(&store_tables);
    // and: a stored scene
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    // when: three updates are applied to it
    for nth in 0..3 {
        store
            .apply(id, &a_paragraph(&format!("Line {nth}.")))
            .await
            .expect("apply should succeed");
    }

    // then: the snapshot and all three updates are left
    assert_eq!(
        bench.rows().await,
        vec![true, false, false, false],
        "keeping three updates means compacting on the fourth, not the third"
    );
}

macro_rules! table_tests {
    ($workbench:ty) => {
        $crate::suite::bench_case!(
            $workbench,
            an_update_is_one_row_and_never_rewrites_the_scene
        );
        $crate::suite::bench_case!(
            $workbench,
            a_tail_longer_than_we_keep_is_collapsed_into_a_snapshot
        );
        $crate::suite::bench_case!($workbench, compacting_a_short_tail_does_nothing);
        $crate::suite::bench_case!(
            $workbench,
            compacting_a_scene_that_was_never_created_is_not_found
        );
        $crate::suite::bench_case!($workbench, an_update_nothing_can_read_writes_no_row);
        $crate::suite::bench_case!(
            $workbench,
            an_update_for_a_scene_that_does_not_exist_writes_no_row
        );
        $crate::suite::bench_case!($workbench, deleting_a_scene_takes_its_updates_with_it);
        $crate::suite::bench_case!(
            $workbench,
            a_scene_survives_being_reloaded_by_a_store_that_never_saw_the_writes
        );
        $crate::suite::bench_case!($workbench, deleting_a_scene_takes_its_links_with_it);
        $crate::suite::bench_case!(
            $workbench,
            a_tail_exactly_as_long_as_we_keep_is_left_as_it_is
        );
    };
}

pub(crate) use {bench_case, conformance_case, conformance_tests, table_tests};
