use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration as Wait;

use acting::{Ending, SessionEnd};
use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum_test::{TestServer, WsMessage};
use clock::FixedClock;
use eventsourcing::{AccountId, Agent};
use live_contract::SESSION_ENDED;
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{FRAGMENT, ScenesCore};
use scenes_store::InMemorySceneStore;
use scenes_sync::{Connection, Envelope, LiveScenes, Message};
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim};

const PROJECT: &str = "project_1";
const OWNER: [Act; 3] = [Act::Read, Act::Write, Act::Manage];
const READER: [Act; 1] = [Act::Read];

#[derive(Default)]
struct Roles {
    held: Mutex<HashMap<String, Vec<Act>>>,
    down: Mutex<bool>,
    changes: AccessChanges,
}

impl Roles {
    fn give(&self, account: &str, acts: &[Act]) {
        self.held
            .lock()
            .expect("never poisoned")
            .insert(account.to_owned(), acts.to_vec());
    }

    fn change(&self, account: &str, acts: &[Act]) {
        self.give(account, acts);
        self.access_changed(PROJECT);
    }
}

#[async_trait]
impl Authorizer for Roles {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }
        let Agent::Account(account) = agent else {
            return Ok(false);
        };

        Ok(project == PROJECT
            && self
                .held
                .lock()
                .expect("never poisoned")
                .get(account.as_str())
                .is_some_and(|acts| acts.contains(&act)))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }

    fn access_changed(&self, project: &str) {
        self.changes.tell(project);
    }

    fn watch_access(&self, project: &str) -> AccessWatch {
        self.changes.watch(project)
    }
}

fn account(name: &str) -> Agent {
    Agent::Account(AccountId::from(name))
}

struct Guarded {
    roles: Arc<Roles>,
    service: ScenesCore,
    live: LiveScenes,
    scene: String,
}

async fn guarded() -> Guarded {
    let roles = Arc::new(Roles::default());
    roles.give("account_ada", &OWNER);
    roles.give("account_bo", &READER);
    let service = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        roles.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    let scene = service
        .create(PROJECT, &account("account_ada"))
        .await
        .expect("the owner may create a scene")
        .id()
        .to_string();

    Guarded {
        roles,
        live: LiveScenes::new(service.clone()),
        service,
        scene,
    }
}

impl Guarded {
    async fn joined_as(&self, name: &str) -> (Connection, UnboundedReceiver<Vec<u8>>) {
        let (to_client, heard) = mpsc::unbounded_channel();
        let mut connection = Connection::new(self.live.clone(), account(name), to_client);
        connection
            .hear(&Envelope::Join(self.scene.clone()).encode())
            .await;

        (connection, heard)
    }

    async fn text(&self) -> String {
        self.service
            .open(&self.scene)
            .await
            .expect("should reopen")
            .text()
    }
}

async fn send(connection: &mut Connection, scene: &str, message: Message) {
    connection
        .hear(
            &Envelope::Carry {
                scene: scene.to_owned(),
                frame: message.encode(),
            }
            .encode(),
        )
        .await;
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

async fn everything(heard: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Message> {
    let mut all = Vec::new();
    while let Some(bytes) = tokio::time::timeout(Wait::from_millis(200), heard.recv())
        .await
        .ok()
        .flatten()
    {
        match Envelope::decode(&bytes).expect("the server sends whole envelopes") {
            Envelope::Carry { frame, .. } => {
                all.push(Message::decode(&frame).expect("a carried frame is a sync message"));
            }
            other => panic!("expected a carried frame, got {other:?}"),
        }
    }

    all
}

async fn joined_on_the_socket_as(guarded: &Guarded, name: &str) -> Envelope {
    let server = TestServer::builder()
        .http_transport()
        .build(scenes_sync::router(guarded.live.clone()).layer(Extension(account(name))));
    let mut socket = server
        .get_websocket("/scenes/sync")
        .await
        .into_websocket()
        .await;
    socket
        .send_message(WsMessage::Binary(
            Envelope::Join(guarded.scene.clone()).encode().into(),
        ))
        .await;
    let bytes = tokio::time::timeout(Wait::from_secs(1), socket.receive_bytes())
        .await
        .expect("an answer while it is fresh");

    Envelope::decode(&bytes).expect("a whole envelope")
}

#[tokio::test]
async fn the_socket_asks_for_whoever_opened_it() {
    let guarded = guarded().await;

    assert_eq!(
        joined_on_the_socket_as(&guarded, "account_grace").await,
        Envelope::Refused(guarded.scene.clone())
    );
    assert!(matches!(
        joined_on_the_socket_as(&guarded, "account_ada").await,
        Envelope::Carry { .. }
    ));
}

#[tokio::test]
async fn whoever_may_not_read_the_project_is_refused_the_scene_by_name() {
    let guarded = guarded().await;

    let (_, mut heard_by_grace) = guarded.joined_as("account_grace").await;

    let bytes = heard_by_grace.recv().await.expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone()),
        "grace is told no more than of a scene that is not there"
    );
}

#[tokio::test]
async fn when_who_may_join_cannot_be_asked_the_scene_is_refused() {
    let guarded = guarded().await;
    *guarded.roles.down.lock().expect("never poisoned") = true;

    let (_, mut heard_by_ada) = guarded.joined_as("account_ada").await;

    let bytes = heard_by_ada.recv().await.expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone())
    );
}

#[tokio::test]
async fn a_reader_joins_asks_what_is_there_and_hears_an_author_s_edits() {
    let guarded = guarded().await;
    let (mut ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    assert!(
        matches!(
            everything(&mut heard_by_bo).await.as_slice(),
            [Message::WhatDoYouHave(_)]
        ),
        "the reader is greeted as an author is"
    );

    send(
        &mut ada,
        &guarded.scene,
        Message::JustHappened(a_paragraph("The loom stood silent.")),
    )
    .await;
    send(&mut bo, &guarded.scene, Message::WhatDoYouHave(vec![0])).await;

    let to_bo = everything(&mut heard_by_bo).await;
    assert!(
        to_bo
            .iter()
            .any(|message| matches!(message, Message::JustHappened(_)))
            && to_bo
                .iter()
                .any(|message| matches!(message, Message::HereIsWhatYouMissed(_))),
        "bo hears the edit and is told what was missed, got {to_bo:?}"
    );
    assert_eq!(guarded.text().await, "The loom stood silent.");
}

#[tokio::test]
async fn what_a_reader_sends_to_change_the_text_goes_nowhere() {
    let guarded = guarded().await;
    let (_ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;

    send(
        &mut bo,
        &guarded.scene,
        Message::JustHappened(a_paragraph("Typed by a reader.")),
    )
    .await;
    send(
        &mut bo,
        &guarded.scene,
        Message::HereIsWhatYouMissed(a_paragraph("Synced by a reader.")),
    )
    .await;

    assert!(everything(&mut heard_by_ada).await.is_empty());
    assert!(everything(&mut heard_by_bo).await.is_empty());
    assert_eq!(guarded.text().await, "");
}

#[tokio::test]
async fn a_reader_is_seen_by_the_others_and_asks_who_is_there() {
    let guarded = guarded().await;
    let (_ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;

    send(&mut bo, &guarded.scene, Message::Awareness(vec![1, 2, 3])).await;
    send(&mut bo, &guarded.scene, Message::WhoIsHere).await;

    assert_eq!(
        everything(&mut heard_by_ada).await,
        vec![Message::Awareness(vec![1, 2, 3]), Message::WhoIsHere],
        "where a reader's cursor is shows for everyone in the scene"
    );
}

async fn asked_again(connection: &mut Connection) {
    let scene = tokio::time::timeout(Wait::from_secs(1), connection.access_changed())
        .await
        .expect("a change to the members is heard");
    connection.ask_again(scene).await;
}

#[tokio::test]
async fn a_member_removed_is_refused_the_scene_they_had_joined_and_heard_no_more() {
    let guarded = guarded().await;
    let (mut ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;

    guarded.roles.change("account_bo", &[]);
    asked_again(&mut bo).await;
    asked_again(&mut ada).await;

    let bytes = tokio::time::timeout(Wait::from_secs(1), heard_by_bo.recv())
        .await
        .expect("told while it is fresh")
        .expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone())
    );
    send(&mut bo, &guarded.scene, Message::Awareness(vec![1, 2, 3])).await;
    send(
        &mut ada,
        &guarded.scene,
        Message::JustHappened(a_paragraph("Not for Bo.")),
    )
    .await;
    assert!(everything(&mut heard_by_ada).await.is_empty());
    assert!(
        everything(&mut heard_by_bo).await.is_empty(),
        "nothing more of the scene reaches whoever left it"
    );
    assert_eq!(guarded.text().await, "Not for Bo.", "the owner writes on");
}

#[tokio::test]
async fn a_reader_made_an_author_writes_and_an_author_made_a_reader_no_longer_does() {
    let guarded = guarded().await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_bo).await;

    guarded.roles.change("account_bo", &[Act::Read, Act::Write]);
    asked_again(&mut bo).await;
    send(
        &mut bo,
        &guarded.scene,
        Message::JustHappened(a_paragraph("Bo writes now.")),
    )
    .await;
    assert_eq!(guarded.text().await, "Bo writes now.");

    guarded.roles.change("account_bo", &READER);
    asked_again(&mut bo).await;
    send(
        &mut bo,
        &guarded.scene,
        Message::JustHappened(a_paragraph("And no more.")),
    )
    .await;
    assert_eq!(guarded.text().await, "Bo writes now.");
}

#[tokio::test]
async fn when_who_may_stay_cannot_be_asked_the_scene_is_refused() {
    let guarded = guarded().await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_bo).await;

    *guarded.roles.down.lock().expect("never poisoned") = true;
    guarded.roles.access_changed(PROJECT);
    asked_again(&mut bo).await;

    let bytes = tokio::time::timeout(Wait::from_secs(1), heard_by_bo.recv())
        .await
        .expect("told while it is fresh")
        .expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone())
    );
}

#[tokio::test]
async fn the_socket_asks_again_when_the_members_change() {
    let guarded = guarded().await;
    let server = TestServer::builder()
        .http_transport()
        .build(scenes_sync::router(guarded.live.clone()).layer(Extension(account("account_bo"))));
    let mut socket = server
        .get_websocket("/scenes/sync")
        .await
        .into_websocket()
        .await;
    socket
        .send_message(WsMessage::Binary(
            Envelope::Join(guarded.scene.clone()).encode().into(),
        ))
        .await;
    _ = tokio::time::timeout(Wait::from_secs(1), socket.receive_bytes())
        .await
        .expect("greeted while it is fresh");

    guarded.roles.change("account_bo", &[]);

    let bytes = tokio::time::timeout(Wait::from_secs(1), socket.receive_bytes())
        .await
        .expect("told while it is fresh");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone())
    );
}

struct Signing(Notify);

#[async_trait]
impl Ending for Signing {
    async fn reached(&self) {
        self.0.notified().await;
    }
}

#[tokio::test]
async fn the_socket_is_closed_as_such_when_its_session_ends() {
    let guarded = guarded().await;
    let signing = Arc::new(Signing(Notify::new()));
    let server = TestServer::builder().http_transport().build(
        scenes_sync::router(guarded.live.clone())
            .layer(Extension(account("account_bo")))
            .layer(Extension(SessionEnd::of(signing.clone()))),
    );
    let mut socket = server
        .get_websocket("/scenes/sync")
        .await
        .into_websocket()
        .await;

    signing.0.notify_one();

    assert!(matches!(
        tokio::time::timeout(Wait::from_secs(1), socket.receive_message()).await,
        Ok(WsMessage::Close(Some(frame))) if u16::from(frame.code) == SESSION_ENDED
    ));
}
