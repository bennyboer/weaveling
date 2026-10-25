use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::Agent;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{
    FRAGMENT, IdeaLink, ProjectLink, Scene, SceneId, SceneStore, SceneTitle, ScenesCore, StoreError,
};
use scenes_store::InMemorySceneStore;
use scenes_sync::{LiveScenes, Message, Presence};
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const A_PROJECT: &str = "project_1";

struct FalteringStore {
    held: InMemorySceneStore,
    faltering: AtomicBool,
    announced: Mutex<Vec<(SceneId, usize)>>,
    openings: Mutex<Vec<(SceneId, String)>>,
}

impl FalteringStore {
    fn falter(&self, faltering: bool) {
        self.faltering.store(faltering, Ordering::SeqCst);
    }
}

#[async_trait]
impl SceneStore for FalteringStore {
    async fn create(&self, scene: &Scene) -> Result<(), StoreError> {
        self.held.create(scene).await
    }

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError> {
        self.held.load(id).await
    }

    async fn apply(&self, id: SceneId, update: &[u8]) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.apply(id, update).await
    }

    async fn retitle(&self, id: SceneId, title: &SceneTitle) -> Result<(), StoreError> {
        self.held.retitle(id, title).await
    }

    async fn announce_word_count(&self, id: SceneId, words: usize) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.announce_word_count(id, words).await?;
        self.announced
            .lock()
            .expect("the announced counts are never poisoned")
            .push((id, words));

        Ok(())
    }

    async fn announce_opening(&self, id: SceneId, opening: &str) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.announce_opening(id, opening).await?;
        self.openings
            .lock()
            .expect("the announced openings are never poisoned")
            .push((id, opening.to_owned()));

        Ok(())
    }

    async fn link(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.link(id, idea).await
    }

    async fn unlink(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.unlink(id, idea).await
    }

    async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.unlink_everywhere(idea).await
    }

    async fn delete(&self, id: SceneId) -> Result<(), StoreError> {
        self.held.delete(id).await
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError> {
        self.held.in_project(project, after, at_most).await
    }
}

struct Workbench {
    store: Arc<FalteringStore>,
    service: ScenesCore,
    live: LiveScenes,
}

fn a_workbench() -> Workbench {
    let store = Arc::new(FalteringStore {
        held: InMemorySceneStore::new(),
        faltering: AtomicBool::new(false),
        announced: Mutex::new(Vec::new()),
        openings: Mutex::new(Vec::new()),
    });
    let service = ScenesCore::new(
        store.clone(),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
        )),
    );

    Workbench {
        store,
        live: LiveScenes::new(service.clone()),
        service,
    }
}

impl Workbench {
    async fn a_scene(&self) -> SceneId {
        self.service
            .create(A_PROJECT, &Agent::Local)
            .await
            .expect("should create")
            .id()
    }

    fn openings_told(&self, id: SceneId) -> Vec<String> {
        self.store
            .openings
            .lock()
            .expect("the announced openings are never poisoned")
            .iter()
            .filter(|(scene, _)| *scene == id)
            .map(|(_, opening)| opening.clone())
            .collect()
    }

    fn last_announced(&self, id: SceneId) -> Option<usize> {
        self.store
            .announced
            .lock()
            .expect("the announced counts are never poisoned")
            .iter()
            .rev()
            .find(|(scene, _)| *scene == id)
            .map(|(_, words)| *words)
    }

    async fn stored_text(&self, id: SceneId) -> String {
        self.service
            .open(&id.to_string())
            .await
            .expect("should reopen")
            .text()
    }
}

fn a_paragraph(saying: &str) -> Vec<u8> {
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

#[tokio::test]
async fn a_scene_stays_live_while_anyone_is_present() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let _present = bench.live.join(id).await.expect("should join");

    bench.live.sweep(Instant::now()).await;

    assert!(
        bench.live.is_live(id),
        "a scene someone is writing in must never be swept"
    );
}

#[tokio::test]
async fn a_scene_everyone_left_stays_live_through_the_grace_period() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let before_leaving = Instant::now();
    drop(bench.live.join(id).await.expect("should join"));

    bench.live.sweep(before_leaving).await;

    assert!(
        bench.live.is_live(id),
        "a reload or a flaky connection must find the same copy it left"
    );
}

#[tokio::test]
async fn a_scene_everyone_left_is_let_go_once_the_grace_period_is_over() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    drop(bench.live.join(id).await.expect("should join"));

    bench.live.sweep(Instant::now()).await;

    assert!(!bench.live.is_live(id));
}

#[tokio::test]
async fn a_scene_rejoined_within_the_grace_period_is_the_same_copy_and_stays() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let first = bench.live.join(id).await.expect("should join");
    let copy = first.scene().clone();
    drop(first);

    let again = bench.live.join(id).await.expect("should join again");
    bench.live.sweep(Instant::now()).await;

    assert!(Arc::ptr_eq(&copy, again.scene()));
    assert!(bench.live.is_live(id));
}

#[tokio::test]
async fn a_scene_is_let_go_only_when_its_last_peer_leaves() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let ada = bench.live.join(id).await.expect("ada should join");
    let bo = bench.live.join(id).await.expect("bo should join");

    drop(ada);
    bench.live.sweep(Instant::now()).await;
    assert!(bench.live.is_live(id), "bo is still writing");

    drop(bo);
    bench.live.sweep(Instant::now()).await;
    assert!(!bench.live.is_live(id));
}

#[tokio::test]
async fn joining_after_a_scene_was_let_go_brings_back_what_was_stored() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let first = bench.live.join(id).await.expect("should join");
    let copy = first.scene().clone();
    first
        .persist(&a_paragraph("The loom stood silent."))
        .await
        .expect("should persist");
    drop(first);
    bench.live.sweep(Instant::now()).await;

    let afresh = bench.live.join(id).await.expect("should join again");

    assert!(!Arc::ptr_eq(&copy, afresh.scene()), "a fresh copy");
    assert_eq!(afresh.text(), "The loom stood silent.");
}

#[tokio::test]
async fn an_edit_the_store_refused_is_flushed_before_the_scene_is_let_go() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&update).await.is_err());
    bench.store.falter(false);
    drop(present);

    bench.live.sweep(Instant::now()).await;

    assert!(!bench.live.is_live(id));
    assert_eq!(
        bench.stored_text(id).await,
        "The loom stood silent.",
        "letting go of the only copy that held an edit would lose it"
    );
}

#[tokio::test]
async fn a_scene_that_cannot_be_flushed_stays_live() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&update).await.is_err());
    drop(present);

    bench.live.sweep(Instant::now()).await;
    assert!(
        bench.live.is_live(id),
        "while the store is down, memory holds the only copy of the edit"
    );

    bench.store.falter(false);
    bench.live.sweep(Instant::now()).await;
    assert!(!bench.live.is_live(id));
    assert_eq!(bench.stored_text(id).await, "The loom stood silent.");
}

#[tokio::test]
async fn a_scene_deleted_while_live_is_let_go_even_with_an_unstored_edit() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&update).await.is_err());
    bench.store.falter(false);
    drop(present);
    bench
        .service
        .delete(&id.to_string(), &Agent::Local)
        .await
        .expect("should delete");

    bench.live.sweep(Instant::now()).await;

    assert!(
        !bench.live.is_live(id),
        "a deleted scene has nowhere to be flushed to, and must not linger forever"
    );
}

async fn writes(present: &Presence, saying: &str) {
    let update = a_paragraph(saying);
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("should write");
    present.persist(&update).await.expect("should persist");
}

#[tokio::test]
async fn the_first_edit_stored_is_counted_at_once() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");

    writes(&present, "The loom stood silent.").await;

    assert_eq!(bench.last_announced(id), Some(4));
}

#[tokio::test(start_paused = true)]
async fn edits_close_behind_are_counted_once_a_short_wait_is_over() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;

    writes(&present, "Then it spoke.").await;

    assert_eq!(
        bench.last_announced(id),
        Some(4),
        "counting every keystroke would cost a write each, so a count waits a moment"
    );
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert_eq!(
        bench.last_announced(id),
        Some(7),
        "the wait ends in a count, so a burst of typing is never left uncounted"
    );
}

#[tokio::test(start_paused = true)]
async fn a_scene_is_counted_before_it_is_let_go() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    writes(&present, "Then it spoke.").await;
    drop(present);

    bench.live.sweep(Instant::now()).await;

    assert!(!bench.live.is_live(id));
    assert_eq!(bench.last_announced(id), Some(7));
}

#[tokio::test(start_paused = true)]
async fn a_count_that_cannot_be_stored_does_not_keep_a_scene_live() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    writes(&present, "Then it spoke.").await;
    drop(present);

    bench.store.falter(true);
    bench.live.sweep(Instant::now()).await;

    assert!(
        !bench.live.is_live(id),
        "the words are worked out from the text, so a missed count is caught up by the          next edit, while holding the scene in memory would last until the store is back"
    );
}

#[tokio::test(start_paused = true)]
async fn settling_before_stopping_counts_what_is_still_waiting_to_be_counted() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    writes(&present, "Then it spoke.").await;
    assert_eq!(bench.last_announced(id), Some(4));

    bench.live.settle().await;

    assert_eq!(
        bench.last_announced(id),
        Some(7),
        "a count waiting out its moment dies with the process, so stopping counts it first"
    );
    assert!(
        bench.live.is_live(id),
        "settling stores and counts, and lets nothing go"
    );
}

#[tokio::test]
async fn settling_before_stopping_stores_an_edit_the_store_refused() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&update).await.is_err());
    bench.store.falter(false);

    bench.live.settle().await;

    assert_eq!(
        bench.stored_text(id).await,
        "The loom stood silent.",
        "the live copy is the only one holding this edit, and stopping would lose it"
    );
}

#[tokio::test(start_paused = true)]
async fn a_scene_s_opening_is_told_when_it_is_counted_and_only_when_it_changed() {
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent in the grey light of morning.");

    for _ in 0..2 {
        present
            .receive(Message::JustHappened(update.clone()))
            .expect("should write");
        present.persist(&update).await.expect("should persist");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    assert_eq!(
        bench.openings_told(id),
        vec!["The loom stood silent in the grey light…".to_owned()],
        "counted twice, but the opening is told once, since it did not change"
    );
}
