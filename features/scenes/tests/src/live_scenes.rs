use std::collections::HashMap;
use std::sync::Arc;

use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::Agent;
use quotation::QuotationMarks;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{FRAGMENT, ProjectLink, Scene, SceneId, SceneRestyler, ScenesCore};
use scenes_store::InMemorySceneStore;
use scenes_sync::{LiveScenes, Message};
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;
use yrs::types::Attrs;
use yrs::{
    Any, Doc, ReadTxn, StateVector, Text, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim,
};

const A_PROJECT: &str = "project_1";

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_workbench() -> (ScenesCore, LiveScenes) {
    let service = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(at(1_000))),
    );

    (service.clone(), LiveScenes::new(service))
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
async fn joining_a_scene_brings_its_stored_prose_back() {
    // given: a scene with stored prose
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();
    service
        .apply(&id, &a_paragraph("The loom stood silent."))
        .await
        .expect("should apply");

    // when: it is joined
    let joined = live.join(scene.id()).await.expect("should join");

    // then: the live scene holds the stored prose
    assert_eq!(
        joined.text(),
        "The loom stood silent.",
        "a live scene must be loaded from the store, not created empty"
    );
}

#[tokio::test]
async fn two_peers_joining_the_same_scene_share_one_document() {
    // given: a scene
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");

    // when: two peers join it
    let ada = live.join(scene.id()).await.expect("ada should join");
    let bo = live.join(scene.id()).await.expect("bo should join");
    // and: one of them writes
    ada.receive(Message::JustHappened(a_paragraph("Ada wrote this.")))
        .expect("ada should write");

    // then: the other sees what was written
    assert_eq!(
        bo.text(),
        "Ada wrote this.",
        "both peers must be looking at the same live document"
    );
}

#[tokio::test]
async fn joining_twice_returns_the_very_same_live_scene() {
    // given: a scene
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");

    // when: it is joined twice
    let first = live.join(scene.id()).await.expect("should join");
    let second = live.join(scene.id()).await.expect("should join");

    // then: both joins hold the same live scene
    assert!(
        Arc::ptr_eq(first.scene(), second.scene()),
        "a second join must reuse the live scene, not build another"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_joins_settle_on_one_live_scene() {
    // given: a scene
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id();

    // when: many join it at once
    let racers: Vec<_> = (0..8)
        .map(|_| {
            let live = live.clone();
            tokio::spawn(async move { live.join(id).await })
        })
        .collect();

    let mut joined = Vec::new();
    for racer in racers {
        joined.push(
            racer
                .await
                .expect("the task should finish")
                .expect("should join"),
        );
    }

    // then: they all hold the same live scene
    let first = &joined[0];
    assert!(
        joined
            .iter()
            .all(|other| Arc::ptr_eq(first.scene(), other.scene())),
        "concurrent joiners must not each get their own copy of the document"
    );
}

#[tokio::test]
async fn two_different_scenes_stay_separate() {
    // given: two scenes, both joined
    let (service, live) = a_workbench();
    let one = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let other = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let first = live.join(one.id()).await.expect("should join");
    let second = live.join(other.id()).await.expect("should join");

    // when: one of them is written to
    first
        .receive(Message::JustHappened(a_paragraph("Only here.")))
        .expect("should write");

    // then: that one holds the writing
    assert_eq!(first.text(), "Only here.");
    // and: the other stays empty
    assert_eq!(second.text(), "", "scenes must not bleed into each other");
}

#[tokio::test]
async fn joining_a_scene_that_does_not_exist_is_refused() {
    // given: a scene nobody created
    let (_service, live) = a_workbench();
    let never_created = SceneId::generate(at(2_000));

    // when: it is joined
    let outcome = live.join(never_created).await;

    // then: the join is refused
    assert!(
        outcome.is_err(),
        "a socket must not open a scene nobody created"
    );
}

#[tokio::test]
async fn every_peer_gets_a_distinct_id() {
    // given: live scenes
    let (_service, live) = a_workbench();

    // when: several peer ids are minted
    let minted: Vec<_> = (0..4).map(|_| live.next_peer()).collect();

    // then: no two are the same
    let mut unique = minted.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), minted.len(), "peer ids must not collide");
}

#[tokio::test]
async fn an_edit_made_in_a_live_scene_is_handed_to_the_store() {
    // given: a joined scene
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();
    let joined = live.join(scene.id()).await.expect("should join");

    // when: an edit is made in it
    let reaction = joined
        .receive(Message::JustHappened(a_paragraph("The loom stood silent.")))
        .expect("should write");
    // and: what it hands to the store is kept
    let update = reaction.to_store.expect("an edit must be durable");
    service.apply(&id, &update).await.expect("should persist");

    // then: the stored scene holds the edit
    let reopened = service.open(&id).await.expect("should reopen");
    assert_eq!(
        reopened.text(),
        "The loom stood silent.",
        "what a peer typed must survive the live scene"
    );
}

#[tokio::test]
async fn a_scene_rejoined_after_everyone_left_still_has_the_prose() {
    // given: a scene written to while joined and then left by everyone
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    let id = scene.id().to_string();
    {
        let joined = live.join(scene.id()).await.expect("should join");
        let reaction = joined
            .receive(Message::JustHappened(a_paragraph("The loom stood silent.")))
            .expect("should write");
        let update = reaction.to_store.expect("an edit must be durable");
        service.apply(&id, &update).await.expect("should persist");
    }

    // when: it is joined again
    let afresh = LiveScenes::new(service);
    let joined = afresh.join(scene.id()).await.expect("should join again");

    // then: the prose is still there
    assert_eq!(joined.text(), "The loom stood silent.");
}

pub(crate) fn a_dialogue() -> Vec<u8> {
    let doc = Doc::new();
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    {
        let mut txn = doc.transact_mut();
        let paragraph = fragment.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        let prose = paragraph.insert(&mut txn, 0, XmlTextPrelim::new(""));
        for (piece, side) in [
            ("»", Some("opening")),
            ("Komm her,", None),
            ("«", Some("closing")),
            (" sagte sie.", None),
        ] {
            let end = prose.len(&txn);
            let attrs = side.map_or_else(Attrs::new, |side| {
                Attrs::from([(
                    Arc::from("quote"),
                    Any::from(HashMap::from([("side".to_owned(), Any::from(side))])),
                )])
            });
            prose.insert_with_attributes(&mut txn, end, piece, attrs);
        }
    }

    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

#[tokio::test]
async fn a_scene_nobody_has_open_is_restyled_where_it_is_kept() {
    // given: a stored scene with dialogue that nobody has open
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    service
        .apply(&scene.id().to_string(), &a_dialogue())
        .await
        .expect("should apply");

    // when: it is restyled
    let restyled = live
        .restyle(scene.id(), QuotationMarks::LowHigh)
        .await
        .expect("should restyle");

    // then: it is reported restyled
    assert!(restyled);
    // and: the stored scene carries the new marks
    let reopened = service
        .open(&scene.id().to_string())
        .await
        .expect("should open");
    assert_eq!(reopened.text(), "„Komm her,“ sagte sie.");
}

#[tokio::test]
async fn whoever_has_the_scene_open_hears_it_restyled_as_it_happens() {
    // given: a scene with dialogue that is open
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    service
        .apply(&scene.id().to_string(), &a_dialogue())
        .await
        .expect("should apply");
    let open = live.join(scene.id()).await.expect("should join");
    let mut hearing = open.listen();
    // and: an editor holding the scene as it was
    let listening = Scene::rehydrate(
        scene.id(),
        ProjectLink::from(A_PROJECT),
        &service
            .open(&scene.id().to_string())
            .await
            .expect("should open")
            .everything(),
    )
    .expect("an editor that has the scene as it was");

    // when: it is restyled
    live.restyle(scene.id(), QuotationMarks::Curly)
        .await
        .expect("should restyle");

    // then: the editor catches up on what it hears
    let heard = hearing
        .try_recv()
        .expect("the change is sent to everyone in the scene");
    let Ok(Message::JustHappened(update)) = Message::decode(&heard.frame) else {
        panic!("a restyling reaches the open scene as something that just happened");
    };
    listening.apply(&update).expect("what was heard applies");
    assert_eq!(
        listening.text(),
        "“Komm her,” sagte sie.",
        "hearing the change is enough to catch up"
    );
    // and: the open scene carries the new marks
    assert_eq!(open.text(), "“Komm her,” sagte sie.");
}

#[tokio::test]
async fn a_scene_already_in_the_style_is_left_alone() {
    // given: a scene with dialogue in pointing-in marks
    let (service, live) = a_workbench();
    let scene = service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create");
    service
        .apply(&scene.id().to_string(), &a_dialogue())
        .await
        .expect("should apply");

    // when: it is restyled to the same marks
    let restyled = live
        .restyle(scene.id(), QuotationMarks::PointingIn)
        .await
        .expect("should restyle");

    // then: nothing is restyled
    assert!(!restyled, "»…« is what the scene already says");
}
