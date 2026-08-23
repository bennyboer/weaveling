use std::sync::Arc;

use clock::FixedClock;
use passages_core::{FRAGMENT, PassageId, PassageService};
use passages_store::InMemoryPassageStore;
use passages_sync::{LivePassages, Message};
use time::{Duration, OffsetDateTime};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_workbench() -> (PassageService, LivePassages) {
    let service = PassageService::new(
        Arc::new(InMemoryPassageStore::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );

    (service.clone(), LivePassages::new(service))
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
async fn hydrating_a_passage_brings_its_stored_prose_back() {
    let (service, live) = a_workbench();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();
    service
        .absorb(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should absorb");

    let hydrated = live.hydrate(passage.id()).await.expect("should hydrate");

    assert_eq!(
        hydrated.room.text(),
        "The loom stood silent.",
        "a live passage must be hydrated from the store, not created empty"
    );
}

#[tokio::test]
async fn two_peers_hydrating_the_same_passage_share_one_document() {
    let (service, live) = a_workbench();
    let passage = service.create().await.expect("should create");

    let ada = live
        .hydrate(passage.id())
        .await
        .expect("ada should hydrate");
    let bo = live.hydrate(passage.id()).await.expect("bo should hydrate");
    ada.room
        .receive(Message::JustHappened(a_paragraph("Ada wrote this.")))
        .expect("ada should write");

    assert_eq!(
        bo.room.text(),
        "Ada wrote this.",
        "both peers must be looking at the same live document"
    );
}

#[tokio::test]
async fn two_different_passages_stay_separate() {
    let (service, live) = a_workbench();
    let one = service.create().await.expect("should create");
    let other = service.create().await.expect("should create");

    let first = live.hydrate(one.id()).await.expect("should hydrate");
    let second = live.hydrate(other.id()).await.expect("should hydrate");
    first
        .room
        .receive(Message::JustHappened(a_paragraph("Only here.")))
        .expect("should write");

    assert_eq!(first.room.text(), "Only here.");
    assert_eq!(
        second.room.text(),
        "",
        "passages must not bleed into each other"
    );
}

#[tokio::test]
async fn hydrating_a_passage_that_does_not_exist_is_refused() {
    let (_service, live) = a_workbench();
    let never_created = PassageId::generate(at(2_000));

    let outcome = live.hydrate(never_created).await;

    assert!(
        outcome.is_err(),
        "a socket must not open a passage nobody created"
    );
}

#[tokio::test]
async fn an_edit_made_in_a_live_passage_is_handed_to_the_store() {
    let (service, live) = a_workbench();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();
    let hydrated = live.hydrate(passage.id()).await.expect("should hydrate");

    let reaction = hydrated
        .room
        .receive(Message::JustHappened(a_paragraph("The loom stood silent.")))
        .expect("should write");
    let update = reaction.to_store.expect("an edit must be durable");
    service.absorb(&id, &update).await.expect("should persist");

    let reopened = service.open(&id).await.expect("should reopen");
    assert_eq!(
        reopened.text(),
        "The loom stood silent.",
        "what a peer typed must survive the live passage"
    );
}

#[tokio::test]
async fn a_passage_rehydrated_after_everyone_left_still_has_the_prose() {
    let (service, live) = a_workbench();
    let passage = service.create().await.expect("should create");
    let id = passage.id().to_string();
    {
        let hydrated = live.hydrate(passage.id()).await.expect("should hydrate");
        let reaction = hydrated
            .room
            .receive(Message::JustHappened(a_paragraph("The loom stood silent.")))
            .expect("should write");
        let update = reaction.to_store.expect("an edit must be durable");
        service.absorb(&id, &update).await.expect("should persist");
    }

    let afresh = LivePassages::new(service);
    let hydrated = afresh
        .hydrate(passage.id())
        .await
        .expect("should hydrate again");

    assert_eq!(hydrated.room.text(), "The loom stood silent.");
}

#[tokio::test]
async fn hydrating_twice_returns_the_very_same_live_passage() {
    let (service, live) = a_workbench();
    let passage = service.create().await.expect("should create");

    let first = live.hydrate(passage.id()).await.expect("should hydrate");
    let second = live.hydrate(passage.id()).await.expect("should hydrate");

    assert!(
        Arc::ptr_eq(&first, &second),
        "a second hydration must reuse the live passage, not build another"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_hydrations_settle_on_one_live_passage() {
    let (service, live) = a_workbench();
    let passage = service.create().await.expect("should create");
    let id = passage.id();

    let racers: Vec<_> = (0..8)
        .map(|_| {
            let live = live.clone();
            tokio::spawn(async move { live.hydrate(id).await })
        })
        .collect();

    let mut hydrated = Vec::new();
    for racer in racers {
        hydrated.push(
            racer
                .await
                .expect("the task should finish")
                .expect("should hydrate"),
        );
    }

    let first = &hydrated[0];
    assert!(
        hydrated.iter().all(|other| Arc::ptr_eq(first, other)),
        "concurrent joiners must not each get their own copy of the document"
    );
}
