use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use authorizing::AllowingEverything;
use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::Agent;
use quotation::QuotationMarks;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{
    FRAGMENT, IdeaLink, ProjectLink, Scene, SceneId, SceneRestyler, SceneStore, SceneTitle,
    ScenesCore, StoreError,
};
use scenes_store::InMemorySceneStore;
use scenes_sync::{LiveScenes, Message, Presence};
use time::{Duration, OffsetDateTime};
use tokio::sync::Notify;
use transacting::{Failure, InMemoryTransactions, Transaction, Transactions};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const A_PROJECT: &str = "project_1";

struct FalteringStore {
    held: InMemorySceneStore,
    faltering: AtomicBool,
    announced: Mutex<Vec<(SceneId, usize)>>,
    openings: Mutex<Vec<(SceneId, String)>>,
    edited_mid_count: Mutex<Option<Vec<u8>>>,
    refusing_openings: AtomicBool,
    held_read: Mutex<Option<HeldRead>>,
}

#[derive(Clone)]
struct HeldRead {
    read: Arc<Notify>,
    let_through: Arc<Notify>,
}

struct StallingTransactions {
    held: InMemoryTransactions,
    stalled: Mutex<Option<Arc<Notify>>>,
}

#[async_trait]
impl Transactions for StallingTransactions {
    async fn begin(&self) -> Result<Transaction, Failure> {
        let stall = self.stalled.lock().expect("never poisoned").take();
        if let Some(stall) = stall {
            stall.notified().await;
        }

        self.held.begin().await
    }
}

impl FalteringStore {
    fn falter(&self, faltering: bool) {
        self.faltering.store(faltering, Ordering::SeqCst);
    }
}

#[async_trait]
impl SceneStore for FalteringStore {
    async fn create(&self, transaction: &mut Transaction, scene: &Scene) -> Result<(), StoreError> {
        self.held.create(transaction, scene).await
    }

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError> {
        let loaded = self.held.load(id).await;
        let held = self.held_read.lock().expect("never poisoned").take();
        if let Some(held) = held {
            held.read.notify_one();
            held.let_through.notified().await;
        }

        loaded
    }

    async fn apply(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        update: &[u8],
    ) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.apply(transaction, id, update).await
    }

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        title: &SceneTitle,
    ) -> Result<(), StoreError> {
        self.held.retitle(transaction, id, title).await
    }

    async fn announce_word_count(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        words: usize,
        as_of: u64,
    ) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }
        self.held
            .announce_word_count(transaction, id, words, as_of)
            .await?;
        self.announced
            .lock()
            .expect("the announced counts are never poisoned")
            .push((id, words));

        Ok(())
    }

    async fn announce_opening(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        opening: &str,
    ) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) || self.refusing_openings.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.announce_opening(transaction, id, opening).await?;
        self.openings
            .lock()
            .expect("the announced openings are never poisoned")
            .push((id, opening.to_owned()));

        Ok(())
    }

    async fn link(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.held.link(transaction, id, idea).await
    }

    async fn unlink(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.held.unlink(transaction, id, idea).await
    }

    async fn unlink_everywhere(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.held.unlink_everywhere(transaction, idea).await
    }

    async fn delete(&self, transaction: &mut Transaction, id: SceneId) -> Result<(), StoreError> {
        self.held.delete(transaction, id).await
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError> {
        self.held.in_project(project, after, at_most).await
    }

    async fn version_of(&self, id: SceneId) -> Result<u64, StoreError> {
        self.held.version_of(id).await
    }

    async fn prose_version_of(&self, id: SceneId) -> Result<u64, StoreError> {
        let version = self.held.prose_version_of(id).await?;
        let edit = self.edited_mid_count.lock().expect("never poisoned").take();
        if let Some(edit) = edit {
            let mut elsewhere = InMemoryTransactions::new()
                .begin()
                .await
                .expect("an in-memory transaction begins");
            self.held.apply(&mut elsewhere, id, &edit).await?;
            elsewhere
                .finish()
                .await
                .expect("an in-memory transaction commits");
        }

        Ok(version)
    }

    async fn version_within(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
    ) -> Result<u64, StoreError> {
        self.held.version_within(transaction, id).await
    }

    async fn uncounted(&self, at_most: usize) -> Result<Vec<SceneId>, StoreError> {
        self.held.uncounted(at_most).await
    }
}

struct Workbench {
    store: Arc<FalteringStore>,
    transactions: Arc<StallingTransactions>,
    service: ScenesCore,
    live: LiveScenes,
}

fn a_workbench() -> Workbench {
    let store = Arc::new(FalteringStore {
        held: InMemorySceneStore::new(),
        faltering: AtomicBool::new(false),
        announced: Mutex::new(Vec::new()),
        openings: Mutex::new(Vec::new()),
        edited_mid_count: Mutex::new(None),
        refusing_openings: AtomicBool::new(false),
        held_read: Mutex::new(None),
    });
    let transactions = Arc::new(StallingTransactions {
        held: InMemoryTransactions::new(),
        stalled: Mutex::new(None),
    });
    let service = ScenesCore::new(
        store.clone(),
        transactions.clone(),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        crate::kept::heard_of_nobody(),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
        )),
    );

    Workbench {
        store,
        transactions,
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

    fn holding_the_next_read(&self) -> HeldRead {
        let held = HeldRead {
            read: Arc::new(Notify::new()),
            let_through: Arc::new(Notify::new()),
        };
        *self.store.held_read.lock().expect("never poisoned") = Some(held.clone());

        held
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
    // given: a scene someone is present in
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let _present = bench.live.join(id).await.expect("should join");

    // when: live scenes are swept
    bench.live.sweep(Instant::now()).await;

    // then: it stays live
    assert!(
        bench.live.is_live(id),
        "a scene someone is writing in must never be swept"
    );
}

#[tokio::test]
async fn a_scene_everyone_left_stays_live_through_the_grace_period() {
    // given: a scene everyone just left
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let before_leaving = Instant::now();
    drop(bench.live.join(id).await.expect("should join"));

    // when: live scenes are swept within the grace period
    bench.live.sweep(before_leaving).await;

    // then: it stays live
    assert!(
        bench.live.is_live(id),
        "a reload or a flaky connection must find the same copy it left"
    );
}

#[tokio::test]
async fn a_scene_everyone_left_is_let_go_once_the_grace_period_is_over() {
    // given: a scene everyone left
    let bench = a_workbench();
    let id = bench.a_scene().await;
    drop(bench.live.join(id).await.expect("should join"));

    // when: live scenes are swept after the grace period
    bench.live.sweep(Instant::now()).await;

    // then: it is let go
    assert!(!bench.live.is_live(id));
}

#[tokio::test]
async fn a_scene_rejoined_within_the_grace_period_is_the_same_copy_and_stays() {
    // given: a scene everyone left
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let first = bench.live.join(id).await.expect("should join");
    let copy = first.scene().clone();
    drop(first);

    // when: it is joined again within the grace period
    let again = bench.live.join(id).await.expect("should join again");
    // and: live scenes are swept
    bench.live.sweep(Instant::now()).await;

    // then: the rejoin holds the same copy
    assert!(Arc::ptr_eq(&copy, again.scene()));
    // and: it stays live
    assert!(bench.live.is_live(id));
}

#[tokio::test]
async fn a_scene_is_let_go_only_when_its_last_peer_leaves() {
    // given: a scene two peers are present in
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let ada = bench.live.join(id).await.expect("ada should join");
    let bo = bench.live.join(id).await.expect("bo should join");

    // when: one of them leaves before a sweep
    drop(ada);
    bench.live.sweep(Instant::now()).await;
    // then: it stays live
    assert!(bench.live.is_live(id), "bo is still writing");

    // when: the last one leaves before a sweep
    drop(bo);
    bench.live.sweep(Instant::now()).await;
    // then: it is let go
    assert!(!bench.live.is_live(id));
}

#[tokio::test]
async fn joining_after_a_scene_was_let_go_brings_back_what_was_stored() {
    // given: a scene with stored prose that was let go
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

    // when: it is joined again
    let afresh = bench.live.join(id).await.expect("should join again");

    // then: the join holds a fresh copy
    assert!(!Arc::ptr_eq(&copy, afresh.scene()), "a fresh copy");
    // and: it holds the stored prose
    assert_eq!(afresh.text(), "The loom stood silent.");
}

#[tokio::test]
async fn a_slow_join_outlived_by_a_copy_written_in_and_let_go_holds_what_was_written() {
    // given: a scene
    let bench = a_workbench();
    let id = bench.a_scene().await;
    // and: a join whose read of it is slow
    let held = bench.holding_the_next_read();
    let live = bench.live.clone();
    let slow = tokio::spawn(async move { live.join(id).await });
    held.read.notified().await;

    // when: meanwhile it is joined, written in and let go
    let quick = bench.live.join(id).await.expect("should join");
    quick
        .persist(&a_paragraph("The loom stood silent."))
        .await
        .expect("should persist");
    drop(quick);
    bench.live.sweep(Instant::now()).await;
    // and: the slow read comes back
    held.let_through.notify_one();
    let joined = slow
        .await
        .expect("the slow join finishes")
        .expect("should join");

    // then: the slow join holds what was written meanwhile
    assert_eq!(
        joined.text(),
        "The loom stood silent.",
        "a copy read before the edit would hide it from everyone writing in the scene until it is let go again"
    );
}

#[tokio::test]
async fn an_edit_the_store_refused_is_flushed_before_the_scene_is_let_go() {
    // given: a scene holding an edit the store refused
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&update).await.is_err());
    // and: the store back
    bench.store.falter(false);
    // and: everyone gone
    drop(present);

    // when: live scenes are swept
    bench.live.sweep(Instant::now()).await;

    // then: it is let go
    assert!(!bench.live.is_live(id));
    // and: the edit is stored
    assert_eq!(
        bench.stored_text(id).await,
        "The loom stood silent.",
        "letting go of the only copy that held an edit would lose it"
    );
}

#[tokio::test]
async fn a_scene_that_cannot_be_flushed_stays_live() {
    // given: a scene everyone left holding an edit the store refused
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

    // when: live scenes are swept while the store is down
    bench.live.sweep(Instant::now()).await;
    // then: it stays live
    assert!(
        bench.live.is_live(id),
        "while the store is down, memory holds the only copy of the edit"
    );

    // when: live scenes are swept once the store is back
    bench.store.falter(false);
    bench.live.sweep(Instant::now()).await;
    // then: it is let go
    assert!(!bench.live.is_live(id));
    // and: the edit is stored
    assert_eq!(bench.stored_text(id).await, "The loom stood silent.");
}

#[tokio::test]
async fn a_scene_deleted_while_live_is_let_go_even_with_an_unstored_edit() {
    // given: a scene everyone left holding an edit the store refused
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
    // and: the scene deleted
    bench
        .service
        .delete(&id.to_string(), None, &Agent::Local)
        .await
        .expect("should delete");

    // when: live scenes are swept
    bench.live.sweep(Instant::now()).await;

    // then: it is let go
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
    // given: a joined scene
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");

    // when: a first edit is stored
    writes(&present, "The loom stood silent.").await;

    // then: its words are counted
    assert_eq!(bench.last_announced(id), Some(4));
}

#[tokio::test(start_paused = true)]
async fn edits_close_behind_are_counted_once_a_short_wait_is_over() {
    // given: a scene with a counted edit
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;

    // when: another edit is stored close behind
    writes(&present, "Then it spoke.").await;

    // then: it is not counted yet
    assert_eq!(
        bench.last_announced(id),
        Some(4),
        "counting every keystroke would cost a write each, so a count waits a moment"
    );
    // when: the short wait is over
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    // then: it is counted
    assert_eq!(
        bench.last_announced(id),
        Some(7),
        "the wait ends in a count, so a burst of typing is never left uncounted"
    );
}

#[tokio::test(start_paused = true)]
async fn a_scene_is_counted_before_it_is_let_go() {
    // given: a scene everyone left with an edit waiting to be counted
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    writes(&present, "Then it spoke.").await;
    drop(present);

    // when: live scenes are swept
    bench.live.sweep(Instant::now()).await;

    // then: it is let go
    assert!(!bench.live.is_live(id));
    // and: the waiting edit is counted
    assert_eq!(bench.last_announced(id), Some(7));
}

#[tokio::test(start_paused = true)]
async fn a_count_that_cannot_be_stored_does_not_keep_a_scene_live() {
    // given: a scene everyone left with an edit waiting to be counted
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    writes(&present, "Then it spoke.").await;
    drop(present);

    // when: live scenes are swept while the store is down
    bench.store.falter(true);
    bench.live.sweep(Instant::now()).await;

    // then: it is let go
    assert!(
        !bench.live.is_live(id),
        "the words are worked out from the text, so a missed count is caught up by the          next edit, while holding the scene in memory would last until the store is back"
    );
}

#[tokio::test(start_paused = true)]
async fn settling_before_stopping_counts_what_is_still_waiting_to_be_counted() {
    // given: a scene with an edit waiting to be counted
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    writes(&present, "Then it spoke.").await;
    assert_eq!(bench.last_announced(id), Some(4));

    // when: live scenes are settled
    bench.live.settle().await;

    // then: the waiting edit is counted
    assert_eq!(
        bench.last_announced(id),
        Some(7),
        "a count waiting out its moment dies with the process, so stopping counts it first"
    );
    // and: the scene stays live
    assert!(
        bench.live.is_live(id),
        "settling stores and counts, and lets nothing go"
    );
}

#[tokio::test]
async fn settling_before_stopping_stores_an_edit_the_store_refused() {
    // given: a scene holding an edit the store refused
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&update).await.is_err());
    // and: the store back
    bench.store.falter(false);

    // when: live scenes are settled
    bench.live.settle().await;

    // then: the edit is stored
    assert_eq!(
        bench.stored_text(id).await,
        "The loom stood silent.",
        "the live copy is the only one holding this edit, and stopping would lose it"
    );
}

#[tokio::test(start_paused = true)]
async fn a_scene_s_opening_is_told_when_it_is_counted_and_only_when_it_changed() {
    // given: a joined scene
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let update = a_paragraph("The loom stood silent in the grey light of morning.");

    // when: the same edit is stored and counted twice
    for _ in 0..2 {
        present
            .receive(Message::JustHappened(update.clone()))
            .expect("should write");
        present.persist(&update).await.expect("should persist");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }

    // then: its opening is told once
    assert_eq!(
        bench.openings_told(id),
        vec!["The loom stood silent in the grey light…".to_owned()],
        "counted twice, but the opening is told once, since it did not change"
    );
}

#[tokio::test(start_paused = true)]
async fn a_count_lost_in_a_crash_is_caught_up_without_another_edit() {
    // given: a scene with prose stored but never counted
    let bench = a_workbench();
    let id = bench.a_scene().await;
    bench
        .service
        .apply(&id.to_string(), &a_paragraph("The loom stood silent."))
        .await
        .expect("the prose is stored, and then the server stops before counting it");

    // when: the uncounted are counted
    let counted = bench
        .live
        .count_the_uncounted(10)
        .await
        .expect("counting should succeed");

    // then: the scene is counted
    assert_eq!(counted, 1);
    assert_eq!(bench.last_announced(id), Some(4));
    // and: its opening is told
    assert_eq!(
        bench.openings_told(id),
        vec!["The loom stood silent.".to_owned()]
    );
    // and: nothing is owed any more
    assert!(
        bench
            .service
            .uncounted(10)
            .await
            .expect("asking should succeed")
            .is_empty()
    );
}

#[tokio::test(start_paused = true)]
async fn a_count_that_cannot_be_stored_stays_owed_until_it_can() {
    // given: a scene with uncounted prose
    let bench = a_workbench();
    let id = bench.a_scene().await;
    bench
        .service
        .apply(&id.to_string(), &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    // and: the store down
    bench.store.falter(true);

    // when: the uncounted are counted
    let counted = bench
        .live
        .count_the_uncounted(10)
        .await
        .expect("a count that fails is no failure of the round");
    bench.store.falter(false);

    // then: nothing is counted
    assert_eq!(counted, 0);
    // and: the scene is still owed
    assert_eq!(
        bench
            .service
            .uncounted(10)
            .await
            .expect("asking should succeed"),
        vec![id],
        "the debt is kept with the prose, so the next round pays it"
    );
    // and: a round once the store is back counts it
    assert_eq!(
        bench
            .live
            .count_the_uncounted(10)
            .await
            .expect("counting should succeed"),
        1
    );
    assert_eq!(bench.last_announced(id), Some(4));
}

#[tokio::test(start_paused = true)]
async fn an_edit_stored_while_a_count_is_under_way_keeps_the_scene_owed() {
    // given: a scene with uncounted prose
    let bench = a_workbench();
    let id = bench.a_scene().await;
    bench
        .service
        .apply(&id.to_string(), &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    // and: an edit that will be stored while it is being counted
    *bench.store.edited_mid_count.lock().expect("never poisoned") =
        Some(a_paragraph("Then it spoke."));

    // when: the uncounted are counted
    bench
        .live
        .count_the_uncounted(10)
        .await
        .expect("counting should succeed");

    // then: the words before the edit are counted
    assert_eq!(bench.last_announced(id), Some(4));
    // and: the scene is still owed
    assert_eq!(
        bench
            .service
            .uncounted(10)
            .await
            .expect("asking should succeed"),
        vec![id],
        "the count read the words before the edit was stored, so it may not clear the edit's mark"
    );
}

#[tokio::test(start_paused = true)]
async fn a_count_whose_opening_cannot_be_told_is_not_kept_either() {
    // given: a scene with uncounted prose
    let bench = a_workbench();
    let id = bench.a_scene().await;
    bench
        .service
        .apply(&id.to_string(), &a_paragraph("The loom stood silent."))
        .await
        .expect("writing should succeed");
    // and: a store refusing openings
    bench.store.refusing_openings.store(true, Ordering::SeqCst);

    // when: the uncounted are counted
    bench
        .live
        .count_the_uncounted(10)
        .await
        .expect("a count that fails is no failure of the round");
    let owed = bench
        .service
        .uncounted(10)
        .await
        .expect("asking should succeed");
    // and: counted again once openings are taken
    bench.store.refusing_openings.store(false, Ordering::SeqCst);
    bench
        .live
        .count_the_uncounted(10)
        .await
        .expect("counting should succeed");

    // then: the scene was still owed after the refused opening
    assert_eq!(
        owed,
        vec![id],
        "the count and the opening land together, or the opening is lost once the mark is gone"
    );
    // and: the opening is told
    assert_eq!(
        bench.openings_told(id),
        vec!["The loom stood silent.".to_owned()]
    );
    // and: nothing is owed any more
    assert!(
        bench
            .service
            .uncounted(10)
            .await
            .expect("asking should succeed")
            .is_empty()
    );
}

#[tokio::test(start_paused = true)]
async fn a_count_overtaken_while_it_stalls_is_never_told_after_the_newer_one() {
    // given: a scene with an edit waiting to be counted
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    writes(&present, "The loom stood silent.").await;
    let update = a_paragraph("Then it spoke.");
    present
        .receive(Message::JustHappened(update.clone()))
        .expect("should write");
    present.persist(&update).await.expect("should persist");
    // and: the next count stalling
    let stall = Arc::new(Notify::new());
    *bench.transactions.stalled.lock().expect("never poisoned") = Some(stall.clone());

    // when: a count starts and stalls
    let older = tokio::spawn({
        let live = bench.live.clone();
        async move { live.settle().await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    // and: a newer edit is counted meanwhile
    writes(&present, "And the weavers listened.").await;
    let newer = tokio::spawn({
        let live = bench.live.clone();
        async move { live.settle().await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    stall.notify_one();
    older.await.expect("the older count ends");
    newer.await.expect("the newer count ends");

    // then: the newest count is the last told
    assert_eq!(
        bench.last_announced(id),
        Some(11),
        "one count at a time, so the newest words are the last told"
    );
}

#[tokio::test]
async fn an_edit_stored_after_one_the_store_refused_stores_the_whole_scene() {
    // given: a scene holding an edit the store refused
    let bench = a_workbench();
    let id = bench.a_scene().await;
    let present = bench.live.join(id).await.expect("should join");
    let refused = a_paragraph("The loom stood silent.");
    present
        .receive(Message::JustHappened(refused.clone()))
        .expect("the live copy takes it");
    bench.store.falter(true);
    assert!(present.persist(&refused).await.is_err());
    // and: the store back
    bench.store.falter(false);

    // when: a next edit is stored
    writes(&present, "Then it spoke.").await;

    // then: the store holds both edits
    let stored = bench.stored_text(id).await;
    assert!(
        stored.contains("The loom stood silent.") && stored.contains("Then it spoke."),
        "the next edit stored carries the one refused before it, so a crash before eviction loses neither, \
         but the store holds {stored:?}"
    );
    // when: everyone leaves and live scenes are swept while the store is down
    drop(present);
    bench.store.falter(true);
    bench.live.sweep(Instant::now()).await;
    // then: it is let go
    assert!(
        !bench.live.is_live(id),
        "stored whole, the scene owes the store nothing more, so it is let go even while the store is down"
    );
}

#[tokio::test]
async fn a_restyle_the_store_refused_is_stored_when_it_is_asked_for_again() {
    // given: a scene with dialogue
    let bench = a_workbench();
    let id = bench.a_scene().await;
    bench
        .service
        .apply(&id.to_string(), &crate::live_scenes::a_dialogue())
        .await
        .expect("writing should succeed");
    // and: a restyle the store refused
    bench.store.falter(true);
    assert!(
        bench
            .live
            .restyle(id, QuotationMarks::LowHigh)
            .await
            .is_err(),
        "a restyle that could not be stored is no restyle, so the page is tried again"
    );
    bench.store.falter(false);

    // when: the restyle is asked for again
    bench
        .live
        .restyle(id, QuotationMarks::LowHigh)
        .await
        .expect("asked again, it should succeed");

    // then: the stored scene carries the new marks
    assert_eq!(
        bench.stored_text(id).await,
        "„Komm her,“ sagte sie.",
        "the live copy already holds the new style, so asking again stores it rather than passing it by"
    );
}
