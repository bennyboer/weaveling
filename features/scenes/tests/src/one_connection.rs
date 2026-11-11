use std::sync::Arc;
use std::time::Duration as Wait;

use authorizing::AllowingEverything;
use belonging::InMemoryBelongings;
use clock::FixedClock;
use eventsourcing::Agent;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{FRAGMENT, ScenesCore};
use scenes_store::InMemorySceneStore;
use scenes_sync::{Connection, Envelope, LiveScenes, Message};
use time::{Duration, OffsetDateTime};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use transacting::InMemoryTransactions;
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const A_PROJECT: &str = "project_1";

fn a_workbench() -> (ScenesCore, LiveScenes) {
    let service = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(
            OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
        )),
    );

    (service.clone(), LiveScenes::new(service))
}

fn a_connection(live: &LiveScenes) -> (Connection, UnboundedReceiver<Vec<u8>>) {
    let (to_client, heard) = mpsc::unbounded_channel();

    (
        Connection::new(live.clone(), Agent::Local, to_client),
        heard,
    )
}

async fn a_scene(service: &ScenesCore) -> String {
    service
        .create(A_PROJECT, &Agent::Local)
        .await
        .expect("should create")
        .id()
        .to_string()
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

async fn join(connection: &mut Connection, scene: &str) {
    connection
        .hear(&Envelope::Join(scene.to_owned()).encode())
        .await;
}

async fn edit(connection: &mut Connection, scene: &str, saying: &str) {
    connection
        .hear(
            &Envelope::Carry {
                scene: scene.to_owned(),
                frame: Message::JustHappened(a_paragraph(saying)).encode(),
            }
            .encode(),
        )
        .await;
}

async fn next(heard: &mut UnboundedReceiver<Vec<u8>>) -> Option<Envelope> {
    tokio::time::timeout(Wait::from_millis(200), heard.recv())
        .await
        .ok()
        .flatten()
        .map(|bytes| Envelope::decode(&bytes).expect("the server sends whole envelopes"))
}

async fn everything(heard: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Envelope> {
    let mut all = Vec::new();
    while let Some(envelope) = next(heard).await {
        all.push(envelope);
    }

    all
}

fn carried(envelope: &Envelope) -> (String, Message) {
    match envelope {
        Envelope::Carry { scene, frame } => (
            scene.clone(),
            Message::decode(frame).expect("a carried frame is a sync message"),
        ),
        other => panic!("expected a carried frame, got {other:?}"),
    }
}

#[tokio::test]
async fn joining_a_scene_greets_the_client_under_that_scene_s_name() {
    // given: a scene and a connection
    let (service, live) = a_workbench();
    let scene = a_scene(&service).await;
    let (mut connection, mut heard) = a_connection(&live);

    // when: the connection joins the scene
    join(&mut connection, &scene).await;

    // then: it is greeted under the scene's name
    let (named, greeting) = carried(&next(&mut heard).await.expect("a greeting"));
    assert_eq!(named, scene);
    // and: the greeting asks what the client has
    assert!(
        matches!(greeting, Message::WhatDoYouHave(_)),
        "the server asks what the client has, as it does on a socket of its own"
    );
}

#[tokio::test]
async fn a_scene_that_cannot_be_joined_is_refused_by_name() {
    // given: a connection
    let (_, live) = a_workbench();
    let (mut connection, mut heard) = a_connection(&live);
    // and: a scene nobody created
    let unknown = scenes_core::SceneId::generate(OffsetDateTime::UNIX_EPOCH).to_string();

    // when: it joins that scene and something that is no scene id
    join(&mut connection, &unknown).await;
    join(&mut connection, "not an id").await;

    // then: each is refused by its name
    assert_eq!(
        everything(&mut heard).await,
        vec![
            Envelope::Refused(unknown),
            Envelope::Refused("not an id".to_owned())
        ]
    );
}

#[tokio::test]
async fn an_edit_reaches_another_connection_in_the_same_scene_and_is_stored() {
    // given: two connections in one scene
    let (service, live) = a_workbench();
    let scene = a_scene(&service).await;
    let other = a_scene(&service).await;
    let (mut ada, mut heard_by_ada) = a_connection(&live);
    let (mut bo, mut heard_by_bo) = a_connection(&live);
    let (mut cy, mut heard_by_cy) = a_connection(&live);
    join(&mut ada, &scene).await;
    join(&mut bo, &scene).await;
    // and: a third in another scene
    join(&mut cy, &other).await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;
    everything(&mut heard_by_cy).await;

    // when: one of the first two edits the scene
    edit(&mut ada, &scene, "The loom stood silent.").await;

    // then: the other in the same scene hears the edit under its name
    let to_bo: Vec<(String, Message)> = everything(&mut heard_by_bo)
        .await
        .iter()
        .map(carried)
        .collect();
    assert!(
        to_bo
            .iter()
            .any(|(named, message)| named == &scene && matches!(message, Message::JustHappened(_))),
        "bo writes in the same scene, so bo hears the edit under its name, got {to_bo:?}"
    );
    // and: the one in another scene hears nothing
    assert!(
        everything(&mut heard_by_cy).await.is_empty(),
        "cy writes in another scene and hears nothing of this one"
    );
    // and: the edit is stored
    assert_eq!(
        service.open(&scene).await.expect("should reopen").text(),
        "The loom stood silent."
    );
}

#[tokio::test]
async fn after_leaving_a_scene_nothing_more_of_it_arrives() {
    // given: two connections in one scene
    let (service, live) = a_workbench();
    let scene = a_scene(&service).await;
    let (mut ada, _) = a_connection(&live);
    let (mut bo, mut heard_by_bo) = a_connection(&live);
    join(&mut ada, &scene).await;
    join(&mut bo, &scene).await;
    everything(&mut heard_by_bo).await;

    // when: one leaves the scene
    bo.hear(&Envelope::Leave(scene.clone()).encode()).await;
    // and: the other edits it
    edit(&mut ada, &scene, "The loom stood silent.").await;

    // then: the one that left hears nothing
    assert!(everything(&mut heard_by_bo).await.is_empty());
}

#[tokio::test]
async fn a_frame_for_a_scene_not_joined_is_ignored() {
    // given: a connection in one scene
    let (service, live) = a_workbench();
    let joined = a_scene(&service).await;
    let scene = a_scene(&service).await;
    let (mut connection, mut heard) = a_connection(&live);
    join(&mut connection, &joined).await;
    everything(&mut heard).await;

    // when: it edits a scene it has not joined
    edit(&mut connection, &scene, "Never joined.").await;

    // then: nothing is heard
    assert!(everything(&mut heard).await.is_empty());
    // and: neither scene changes
    for untouched in [&scene, &joined] {
        assert_eq!(
            service.open(untouched).await.expect("should reopen").text(),
            "",
            "a frame lands only in the scene it names, and only once that scene is joined"
        );
    }
}

#[tokio::test]
async fn one_connection_carries_many_scenes_at_once() {
    // given: two scenes and a connection
    let (service, live) = a_workbench();
    let first = a_scene(&service).await;
    let second = a_scene(&service).await;
    let (mut connection, mut heard) = a_connection(&live);

    // when: the connection joins both
    join(&mut connection, &first).await;
    join(&mut connection, &second).await;
    // and: edits each
    edit(&mut connection, &first, "First.").await;
    edit(&mut connection, &second, "Second.").await;

    // then: it is greeted in both
    let greeted: Vec<String> = everything(&mut heard)
        .await
        .iter()
        .map(|envelope| carried(envelope).0)
        .collect();
    assert_eq!(greeted, vec![first.clone(), second.clone()]);
    // and: each scene stores its own edit
    assert_eq!(
        service.open(&first).await.expect("reopens").text(),
        "First."
    );
    assert_eq!(
        service.open(&second).await.expect("reopens").text(),
        "Second."
    );
}

#[tokio::test]
async fn an_edit_into_a_scene_deleted_while_live_refuses_it_and_nobody_joins_it_again() {
    // given: a connection in a scene
    let (service, live) = a_workbench();
    let scene = a_scene(&service).await;
    let (mut writing, mut heard) = a_connection(&live);
    join(&mut writing, &scene).await;
    everything(&mut heard).await;
    // and: the scene deleted
    service
        .delete(&scene, None, &Agent::Local)
        .await
        .expect("deleting should succeed");

    // when: the connection edits it
    edit(&mut writing, &scene, "Too late.").await;
    let told = everything(&mut heard).await;
    // and: another connection joins it
    let (mut joining, mut joined) = a_connection(&live);
    join(&mut joining, &scene).await;

    // then: the edit is refused
    assert!(
        told.contains(&Envelope::Refused(scene.clone())),
        "an edit the store cannot take is not silently kept for an eviction that will drop it"
    );
    // and: the join is refused
    assert_eq!(
        everything(&mut joined).await,
        vec![Envelope::Refused(scene)],
        "a room whose scene is gone lets nobody in"
    );
}
