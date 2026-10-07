use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use async_trait::async_trait;
use clock::FixedClock;
use passages_core::{
    FRAGMENT, IdeaLink, Passage, PassageId, PassageService, PassageStore, PassageTitle,
    ProjectLink, StoreError,
};
use passages_store::InMemoryPassageStore;
use passages_sync::{LivePassages, Message};
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const A_PROJECT: &str = "project_1";

struct FalteringStore {
    held: InMemoryPassageStore,
    faltering: AtomicBool,
}

impl FalteringStore {
    fn falter(&self, faltering: bool) {
        self.faltering.store(faltering, Ordering::SeqCst);
    }
}

#[async_trait]
impl PassageStore for FalteringStore {
    async fn create(&self, passage: &Passage) -> Result<(), StoreError> {
        self.held.create(passage).await
    }

    async fn load(&self, id: PassageId) -> Result<Passage, StoreError> {
        self.held.load(id).await
    }

    async fn apply(&self, id: PassageId, update: &[u8]) -> Result<(), StoreError> {
        if self.faltering.load(Ordering::SeqCst) {
            return Err(StoreError::Backend("the store is down".into()));
        }

        self.held.apply(id, update).await
    }

    async fn retitle(&self, id: PassageId, title: &PassageTitle) -> Result<(), StoreError> {
        self.held.retitle(id, title).await
    }

    async fn link(&self, id: PassageId, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.link(id, idea).await
    }

    async fn unlink(&self, id: PassageId, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.unlink(id, idea).await
    }

    async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError> {
        self.held.unlink_everywhere(idea).await
    }

    async fn delete(&self, id: PassageId) -> Result<(), StoreError> {
        self.held.delete(id).await
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<PassageId>,
        at_most: usize,
    ) -> Result<Vec<PassageId>, StoreError> {
        self.held.in_project(project, after, at_most).await
    }
}

struct Workbench {
    store: Arc<FalteringStore>,
    service: PassageService,
    live: LivePassages,
}

fn a_workbench() -> Workbench {
    let store = Arc::new(FalteringStore {
        held: InMemoryPassageStore::new(),
        faltering: AtomicBool::new(false),
    });
    let service = PassageService::new(
        store.clone(),
        Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
        )),
    );

    Workbench {
        store,
        live: LivePassages::new(service.clone()),
        service,
    }
}

impl Workbench {
    async fn a_passage(&self) -> PassageId {
        self.service
            .create(A_PROJECT)
            .await
            .expect("should create")
            .id()
    }

    async fn stored_text(&self, id: PassageId) -> String {
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
async fn a_passage_stays_live_while_anyone_is_present() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
    let _present = bench.live.join(id).await.expect("should join");

    bench.live.sweep(Instant::now()).await;

    assert!(
        bench.live.is_live(id),
        "a passage someone is writing in must never be swept"
    );
}

#[tokio::test]
async fn a_passage_everyone_left_stays_live_through_the_grace_period() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
    let before_leaving = Instant::now();
    drop(bench.live.join(id).await.expect("should join"));

    bench.live.sweep(before_leaving).await;

    assert!(
        bench.live.is_live(id),
        "a reload or a flaky connection must find the same copy it left"
    );
}

#[tokio::test]
async fn a_passage_everyone_left_is_let_go_once_the_grace_period_is_over() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
    drop(bench.live.join(id).await.expect("should join"));

    bench.live.sweep(Instant::now()).await;

    assert!(!bench.live.is_live(id));
}

#[tokio::test]
async fn a_passage_rejoined_within_the_grace_period_is_the_same_copy_and_stays() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
    let first = bench.live.join(id).await.expect("should join");
    let copy = first.passage().clone();
    drop(first);

    let again = bench.live.join(id).await.expect("should join again");
    bench.live.sweep(Instant::now()).await;

    assert!(Arc::ptr_eq(&copy, again.passage()));
    assert!(bench.live.is_live(id));
}

#[tokio::test]
async fn a_passage_is_let_go_only_when_its_last_peer_leaves() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
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
async fn joining_after_a_passage_was_let_go_brings_back_what_was_stored() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
    let first = bench.live.join(id).await.expect("should join");
    let copy = first.passage().clone();
    first
        .persist(&a_paragraph("The loom stood silent."))
        .await
        .expect("should persist");
    drop(first);
    bench.live.sweep(Instant::now()).await;

    let afresh = bench.live.join(id).await.expect("should join again");

    assert!(!Arc::ptr_eq(&copy, afresh.passage()), "a fresh copy");
    assert_eq!(afresh.text(), "The loom stood silent.");
}

#[tokio::test]
async fn an_edit_the_store_refused_is_flushed_before_the_passage_is_let_go() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
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
async fn a_passage_that_cannot_be_flushed_stays_live() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
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
async fn a_passage_deleted_while_live_is_let_go_even_with_an_unstored_edit() {
    let bench = a_workbench();
    let id = bench.a_passage().await;
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
        .delete(&id.to_string())
        .await
        .expect("should delete");

    bench.live.sweep(Instant::now()).await;

    assert!(
        !bench.live.is_live(id),
        "a deleted passage has nowhere to be flushed to, and must not linger forever"
    );
}
