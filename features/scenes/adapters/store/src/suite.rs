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
        let as_of = self.store.version_of(id).await?;
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
    let id = an_id(1_000);

    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    let found = store.load(id).await.expect("load should find the scene");

    assert_eq!(found.text(), "The loom stood silent.");
    assert_eq!(found.id(), id);
}

pub async fn an_empty_scene_can_be_stored(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);

    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "");
}

pub async fn load_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    let missing = an_id(1_000);

    let error = store
        .load(missing)
        .await
        .expect_err("load should not find the scene");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn create_rejects_a_duplicate_id(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("first create should succeed");

    let error = store
        .create(&a_scene(id, "Something else entirely."))
        .await
        .expect_err("second create should conflict");

    assert!(
        matches!(&error, StoreError::Conflict(existing) if *existing == id),
        "expected Conflict({id}), got {error:?}"
    );
}

pub async fn a_rejected_create_leaves_the_stored_scene_untouched(
    store: &Committing<'_, impl SceneStore>,
) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("first create should succeed");

    let _ = store.create(&a_scene(id, "Something else entirely.")).await;

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorb_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    let missing = an_id(1_000);

    let error = store
        .apply(missing, &a_paragraph("into the void"))
        .await
        .expect_err("apply should not find the scene");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn absorbed_updates_are_visible_on_load(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn absorbing_the_same_update_twice_changes_nothing(
    store: &Committing<'_, impl SceneStore>,
) {
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");
    let update = a_paragraph("The loom stood silent.");

    store.apply(id, &update).await.expect("first apply");
    store.apply(id, &update).await.expect("second apply");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.text(),
        "The loom stood silent.",
        "a repeated update must not duplicate the prose"
    );
}

pub async fn updates_absorbed_in_either_order_converge(store: &Committing<'_, impl SceneStore>) {
    let ada = a_paragraph("Ada wrote this.");
    let bo = a_paragraph("Bo wrote this.");
    let one = an_id(1_000);
    let other = an_id(2_000);
    for id in [one, other] {
        store
            .create(&Scene::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    store.apply(one, &ada).await.expect("apply ada");
    store.apply(one, &bo).await.expect("apply bo");
    store.apply(other, &bo).await.expect("apply bo");
    store.apply(other, &ada).await.expect("apply ada");

    let first = store.load(one).await.expect("load one").text();
    let second = store.load(other).await.expect("load other").text();
    assert_eq!(first, second, "apply order must not matter");
    assert!(first.contains("Ada wrote this."));
    assert!(first.contains("Bo wrote this."));
}

pub async fn an_unusable_update_is_refused_and_changes_nothing(
    store: &Committing<'_, impl SceneStore>,
) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    let error = store
        .apply(id, &[255, 255, 255, 255])
        .await
        .expect_err("garbage must not be accepted");

    assert!(
        matches!(&error, StoreError::Unusable(bad) if *bad == id),
        "expected Unusable({id}), got {error:?}"
    );
    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.text(),
        "The loom stood silent.",
        "a refused update must leave the scene as it was"
    );
}

pub async fn delete_removes_the_scene(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    store.delete(id).await.expect("delete should succeed");

    let found = store.load(id).await;
    assert!(
        matches!(&found, Err(StoreError::NotFound(gone)) if *gone == id),
        "expected NotFound({id}) after delete, got {found:?}"
    );
}

pub async fn delete_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    let missing = an_id(1_000);

    let error = store
        .delete(missing)
        .await
        .expect_err("delete should not find the scene");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn a_loaded_scene_still_knows_its_project(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    let found = store.load(id).await.expect("load should find the scene");

    assert_eq!(found.project(), &a_project());
}

pub async fn a_project_lists_its_own_scenes_in_order(store: &Committing<'_, impl SceneStore>) {
    let first = an_id(1_000);
    let second = an_id(2_000);
    for id in [second, first] {
        store
            .create(&Scene::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    let found = store
        .in_project(&a_project(), None, 10)
        .await
        .expect("listing should succeed");

    assert_eq!(found, vec![first, second]);
}

pub async fn another_project_sees_none_of_them(store: &Committing<'_, impl SceneStore>) {
    let mine = an_id(1_000);
    store
        .create(&Scene::empty(mine, a_project()))
        .await
        .expect("create should succeed");

    let found = store
        .in_project(&ProjectLink::from("project_2"), None, 10)
        .await
        .expect("listing should succeed");

    assert!(
        found.is_empty(),
        "a sweep must never reach into another author's project, got {found:?}"
    );
}

pub async fn listing_resumes_after_the_cursor(store: &Committing<'_, impl SceneStore>) {
    let first = an_id(1_000);
    let second = an_id(2_000);
    let third = an_id(3_000);
    for id in [first, second, third] {
        store
            .create(&Scene::empty(id, a_project()))
            .await
            .expect("create should succeed");
    }

    let batch = store
        .in_project(&a_project(), None, 2)
        .await
        .expect("first batch should succeed");
    let rest = store
        .in_project(&a_project(), batch.last().copied(), 2)
        .await
        .expect("second batch should succeed");

    assert_eq!(batch, vec![first, second]);
    assert_eq!(
        rest,
        vec![third],
        "without an exclusive cursor a sweep hands itself the same batch forever"
    );
}

pub async fn listing_past_the_last_scene_is_empty(store: &Committing<'_, impl SceneStore>) {
    let only = an_id(1_000);
    store
        .create(&Scene::empty(only, a_project()))
        .await
        .expect("create should succeed");

    let found = store
        .in_project(&a_project(), Some(only), 10)
        .await
        .expect("listing should succeed");

    assert!(found.is_empty(), "expected nothing after the last id");
}

pub async fn a_deleted_scene_leaves_the_listing(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    store.delete(id).await.expect("delete should succeed");

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
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    let found = store.load(id).await.expect("load should find the scene");

    assert!(found.title().is_untitled());
}

pub async fn a_retitled_scene_keeps_its_title(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");

    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.title(), &a_title("The loom"));
}

pub async fn retitling_a_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    let missing = an_id(1_000);

    let error = store
        .retitle(missing, &a_title("Nowhere"))
        .await
        .expect_err("retitle should not find the scene");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn retitling_leaves_the_text_as_it_was(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn a_title_survives_further_writing(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&Scene::empty(id, a_project()))
        .await
        .expect("create should succeed");
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

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
    let missing = an_id(1_000);

    let error = store
        .announce_word_count(missing, 42)
        .await
        .expect_err("announcing should not find the scene");

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
    let (written, quiet) = (an_id(1_000), an_id(2_000));
    an_empty_scene(store, written).await;
    an_empty_scene(store, quiet).await;
    assert!(
        uncounted(store).await.is_empty(),
        "a scene only made owes no count"
    );

    store
        .apply(written, &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    assert_eq!(
        uncounted(store).await,
        vec![written],
        "the prose and the count it owes are kept together, so a crash cannot lose the debt"
    );

    store
        .announce_word_count(written, 4)
        .await
        .expect("announcing should succeed");
    assert!(uncounted(store).await.is_empty());
}

pub async fn a_count_as_of_an_older_version_leaves_the_newer_writing_owed(
    store: &Committing<'_, impl SceneStore>,
) {
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    let counted_as_of = store.version_of(id).await.expect("the scene has a version");
    store
        .apply(id, &a_paragraph("Then it spoke."))
        .await
        .expect("writing should succeed");

    store
        .announce_word_count_as_of(id, 4, counted_as_of)
        .await
        .expect("announcing should succeed");

    assert_eq!(
        uncounted(store).await,
        vec![id],
        "what was written after the words were read is not in the count, so it is still owed"
    );
    store
        .announce_word_count(id, 7)
        .await
        .expect("announcing should succeed");
    assert!(uncounted(store).await.is_empty());
}

pub async fn the_version_of_a_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    let missing = an_id(1_000);

    let error = store
        .version_of(missing)
        .await
        .expect_err("there is no version of what is not there");

    assert!(matches!(&error, StoreError::NotFound(id) if *id == missing));
}

pub async fn a_write_rolled_back_owes_no_count(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    let mut transaction = store.begun().await.expect("beginning should succeed");
    store
        .uncommitted()
        .apply(&mut transaction, id, &a_paragraph("Never kept."))
        .await
        .expect("writing should succeed");
    drop(transaction);

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
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    let found = store.load(id).await.expect("load should find the scene");

    assert!(found.ideas().is_empty());
}

pub async fn linked_ideas_come_back_in_the_order_they_were_linked(
    store: &Committing<'_, impl SceneStore>,
) {
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    for idea in ["idea_c", "idea_a", "idea_b"] {
        store
            .link(id, &an_idea(idea))
            .await
            .expect("link should succeed");
    }

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.ideas(),
        &[an_idea("idea_c"), an_idea("idea_a"), an_idea("idea_b")],
        "the author reads this list as the order they reached for the ideas, not the order \
         the ids happen to sort in"
    );
}

pub async fn linking_the_same_idea_twice_is_one_link(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    for _ in 0..2 {
        store
            .link(id, &an_idea("idea_1"))
            .await
            .expect("a repeated link is not a failure");
    }

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.ideas(), &[an_idea("idea_1")]);
}

pub async fn unlinking_takes_away_only_that_idea(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    for idea in ["idea_1", "idea_2"] {
        store
            .link(id, &an_idea(idea))
            .await
            .expect("link should succeed");
    }

    store
        .unlink(id, &an_idea("idea_1"))
        .await
        .expect("unlink should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.ideas(), &[an_idea("idea_2")]);
}

pub async fn unlinking_an_idea_never_linked_is_harmless(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    store
        .unlink(id, &an_idea("idea_elsewhere"))
        .await
        .expect("nothing to unlink is not a failure");
}

pub async fn linking_to_a_missing_scene_is_not_found(store: &Committing<'_, impl SceneStore>) {
    let missing = an_id(1_000);

    let linked = store.link(missing, &an_idea("idea_1")).await;
    let unlinked = store.unlink(missing, &an_idea("idea_1")).await;

    assert!(
        matches!(&linked, Err(StoreError::NotFound(id)) if *id == missing),
        "expected NotFound({missing}) on link, got {linked:?}"
    );
    assert!(
        matches!(&unlinked, Err(StoreError::NotFound(id)) if *id == missing),
        "expected NotFound({missing}) on unlink, got {unlinked:?}"
    );
}

pub async fn unlinking_an_idea_everywhere_reaches_every_scene(
    store: &Committing<'_, impl SceneStore>,
) {
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

    store
        .unlink_everywhere(&an_idea("idea_gone"))
        .await
        .expect("unlinking everywhere should succeed");

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
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(
        found.ideas(),
        &[an_idea("idea_1")],
        "writing rewrites the stored scene, and the links must ride along"
    );
}

pub async fn linking_leaves_the_title_and_the_text_alone(store: &Committing<'_, impl SceneStore>) {
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");
    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    let found = store.load(id).await.expect("load should find the scene");
    assert_eq!(found.title(), &a_title("The loom"));
    assert_eq!(found.text(), "The loom stood silent.");
}

pub async fn linking_an_idea_enqueues_one_message(bench: &impl Workbench) {
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    for _ in 0..2 {
        store
            .link(id, &an_idea("idea_1"))
            .await
            .expect("link should succeed");
    }

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
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;
    store
        .link(id, &an_idea("idea_1"))
        .await
        .expect("link should succeed");

    store
        .unlink(id, &an_idea("idea_1"))
        .await
        .expect("unlink should succeed");
    store
        .unlink(id, &an_idea("idea_never"))
        .await
        .expect("unlinking nothing should succeed");

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

pub async fn every_change_to_a_scene_counts_up(bench: &impl Workbench) {
    let store = &bench.committing();
    let id = an_id(1_000);
    let other = an_id(2_000);
    an_empty_scene(store, id).await;
    an_empty_scene(store, other).await;

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
            (id.to_string(), 4),
            (id.to_string(), 5),
        ],
        "every change to a scene counts, announced or not — retitling and writing too — each          scene on its own, and what changed nothing does not"
    );
}

pub async fn announcing_a_word_count_enqueues_it_with_the_scene_s_project(bench: &impl Workbench) {
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    store
        .announce_word_count(id, 42)
        .await
        .expect("announcing should succeed");

    assert_eq!(
        told_after_creating(bench.enqueued().await),
        vec![(
            "scene.words.counted".to_owned(),
            json!({ "scene": id.to_string(), "project": a_project().to_string(), "words": 42 })
        )]
    );
}

pub async fn announcing_a_word_count_is_not_a_change_to_the_scene(bench: &impl Workbench) {
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

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
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    store.delete(id).await.expect("delete should succeed");

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
    let store = &bench.committing();
    let missing = an_id(1_000);

    let _ = store.link(missing, &an_idea("idea_1")).await;
    let _ = store.unlink(missing, &an_idea("idea_1")).await;
    let _ = store.delete(missing).await;

    assert!(
        bench.enqueued().await.is_empty(),
        "an message_for of something that did not happen sends the listener after a ghost"
    );
}

pub async fn writing_enqueues_nothing(bench: &impl Workbench) {
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    store
        .apply(id, &a_paragraph("The loom stood silent."))
        .await
        .expect("apply should succeed");
    store
        .unlink_everywhere(&an_idea("idea_1"))
        .await
        .expect("unlinking everywhere should succeed");

    assert!(
        told_after_creating(bench.enqueued().await).is_empty(),
        "every keystroke is an edit, so text is announced by what is counted from it, not edit          by edit; and unlinking a discarded idea everywhere is answered by whoever heard the          discard itself"
    );
}

pub async fn creating_a_scene_is_announced_with_its_project(bench: &impl Workbench) {
    let store = &bench.committing();
    let id = an_id(1_000);

    an_empty_scene(store, id).await;

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
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    store
        .retitle(id, &a_title("The loom"))
        .await
        .expect("retitle should succeed");

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
    let store = &bench.committing();
    let id = an_id(1_000);
    an_empty_scene(store, id).await;

    store
        .announce_opening(id, "The loom stood silent.")
        .await
        .expect("announcing should succeed");

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
    let missing = an_id(1_000);

    let error = store
        .announce_opening(missing, "Nowhere.")
        .await
        .expect_err("announcing should not find the scene");

    assert!(
        matches!(&error, StoreError::NotFound(id) if *id == missing),
        "expected NotFound({missing}), got {error:?}"
    );
}

pub async fn what_was_written_in_a_transaction_that_is_dropped_is_gone_and_told_to_nobody(
    bench: &impl Workbench,
) {
    let store = &bench.committing();
    let kept = an_id(1_000);
    store
        .create(&a_scene(kept, "The loom stood silent."))
        .await
        .expect("creating should succeed");
    let told_before = bench.enqueued().await.len();
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

    drop(transaction);

    let scene = store.load(kept).await.expect("the kept scene is there");
    assert_eq!(scene.text(), "The loom stood silent.");
    assert!(scene.ideas().is_empty());
    assert!(matches!(
        store.load(an_id(2_000)).await,
        Err(StoreError::NotFound(_))
    ));
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
        $crate::suite::bench_case!($workbench, every_change_to_a_scene_counts_up);
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

    assert_eq!(
        bench.rows().await,
        vec![true, false],
        "a snapshot from create, then one appended update — the scene itself is never rewritten"
    );
}

pub async fn a_tail_longer_than_we_keep_is_collapsed_into_a_snapshot(bench: &impl Tables) {
    let store_tables = bench.on_the_same_tables(3);
    let store = &bench.committing_to(&store_tables);
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    for nth in 0..5 {
        store
            .apply(id, &a_paragraph(&format!("Line {nth}.")))
            .await
            .expect("apply should succeed");
    }

    let parts = bench.rows().await;

    assert!(
        parts.len() < 6,
        "six writes should not have left six rows: {parts:?}"
    );
    assert_eq!(
        parts.first(),
        Some(&true),
        "what remains must begin with a snapshot: {parts:?}"
    );

    let found = store.load(id).await.expect("load should succeed");
    assert!(
        found.text().contains("The loom stood silent.") && found.text().contains("Line 4."),
        "compaction must not lose prose, got {:?}",
        found.text()
    );
}

pub async fn compacting_a_short_tail_does_nothing(bench: &impl Tables) {
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

    let collapsed = bench.compact(id).await.expect("compacting should succeed");

    assert!(!collapsed, "two rows are not worth collapsing");
    assert_eq!(bench.rows().await.len(), 2);
}

pub async fn compacting_a_scene_that_was_never_created_is_not_found(bench: &impl Tables) {
    let never_written = an_id(1_000);

    let refused = bench.compact(never_written).await;

    assert!(
        matches!(refused, Err(StoreError::NotFound(missing)) if missing == never_written),
        "got {refused:?}"
    );
}

pub async fn an_update_nothing_can_read_writes_no_row(bench: &impl Tables) {
    let id = an_id(1_000);
    bench
        .committing()
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    let refused = bench.committing().apply(id, &[255, 255, 255, 255]).await;

    assert!(
        matches!(refused, Err(StoreError::Unusable(_))),
        "got {refused:?}"
    );
    assert_eq!(
        bench.rows().await,
        vec![true],
        "garbage must not reach the table, or every load would fail forever after"
    );
}

pub async fn an_update_for_a_scene_that_does_not_exist_writes_no_row(bench: &impl Tables) {
    let refused = bench
        .committing()
        .apply(an_id(1_000), &a_paragraph("Out of nowhere."))
        .await;

    assert!(
        matches!(refused, Err(StoreError::NotFound(_))),
        "got {refused:?}"
    );
    assert!(bench.rows().await.is_empty());
}

pub async fn deleting_a_scene_takes_its_updates_with_it(bench: &impl Tables) {
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

    bench
        .committing()
        .delete(id)
        .await
        .expect("delete should succeed");

    assert_eq!(bench.count("SELECT count(*) FROM scenes").await, 0);
    assert!(
        bench.rows().await.is_empty(),
        "the parts are the scene — leaving them would leave a scene that load cannot find"
    );
}

pub async fn a_scene_survives_being_reloaded_by_a_store_that_never_saw_the_writes(
    bench: &impl Tables,
) {
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

    let elsewhere_tables = bench.on_the_same_tables(COMPACT_AFTER);
    let elsewhere = &bench.committing_to(&elsewhere_tables);
    let found = elsewhere.load(id).await.expect("load should succeed");

    assert!(
        found.text().contains("The loom stood silent.") && found.text().contains("Then it began."),
        "prose has to come back out of the rows, not out of whoever wrote them, got {:?}",
        found.text()
    );
}

pub async fn deleting_a_scene_takes_its_links_with_it(bench: &impl Tables) {
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

    bench
        .committing()
        .delete(id)
        .await
        .expect("delete should succeed");

    let links = bench.count("SELECT count(*) FROM scene_ideas").await;
    assert_eq!(
        links, 0,
        "a link outliving its scene points at nothing, and nothing sweeps it later"
    );
}

pub async fn a_tail_exactly_as_long_as_we_keep_is_left_as_it_is(bench: &impl Tables) {
    let store_tables = bench.on_the_same_tables(3);
    let store = &bench.committing_to(&store_tables);
    let id = an_id(1_000);
    store
        .create(&a_scene(id, "The loom stood silent."))
        .await
        .expect("create should succeed");

    for nth in 0..3 {
        store
            .apply(id, &a_paragraph(&format!("Line {nth}.")))
            .await
            .expect("apply should succeed");
    }

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
