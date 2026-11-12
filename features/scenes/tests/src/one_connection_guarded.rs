use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration as Wait, Instant};

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum_test::{TestServer, WsMessage};
use belonging::InMemoryBelongings;
use clock::FixedClock;
use ending::{Ending, SessionEnd};
use eventsourcing::{AccountId, Agent};
use live_contract::{SERVER_CLOSING, SESSION_ENDED};
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{FRAGMENT, ScenesCore};
use scenes_store::InMemorySceneStore;
use scenes_sync::{Connection, Envelope, LARGEST_SENT, LiveScenes, Message};
use time::OffsetDateTime;
use tokio::sync::Notify;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use transacting::InMemoryTransactions;
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

async fn a_guarded_scene() -> Guarded {
    let roles = Arc::new(Roles::default());
    roles.give("account_ada", &OWNER);
    roles.give("account_bo", &READER);
    let service = ScenesCore::new(
        Arc::new(InMemorySceneStore::new()),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemorySceneCatalog::new()),
        Arc::new(InMemoryQuotationRestylingTasks::new()),
        Arc::new(InMemoryBelongings::new()),
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
    // given: a scene in a project with members
    let guarded = a_guarded_scene().await;

    // then: a non-member joining on the socket is refused
    assert_eq!(
        joined_on_the_socket_as(&guarded, "account_grace").await,
        Envelope::Refused(guarded.scene.clone())
    );
    // and: the owner joining on the socket is greeted
    assert!(matches!(
        joined_on_the_socket_as(&guarded, "account_ada").await,
        Envelope::Carry { .. }
    ));
}

#[tokio::test]
async fn whoever_may_not_read_the_project_is_refused_the_scene_by_name() {
    // given: a scene in a project with members
    let guarded = a_guarded_scene().await;

    // when: a non-member joins it
    let (_, mut heard_by_grace) = guarded.joined_as("account_grace").await;

    // then: the scene is refused by name
    let bytes = heard_by_grace.recv().await.expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone()),
        "grace is told no more than of a scene that is not there"
    );
}

#[tokio::test]
async fn when_who_may_join_cannot_be_asked_the_scene_is_refused() {
    // given: a scene in a project with members
    let guarded = a_guarded_scene().await;
    // and: who may join cannot be asked
    *guarded.roles.down.lock().expect("never poisoned") = true;

    // when: the owner joins it
    let (_, mut heard_by_ada) = guarded.joined_as("account_ada").await;

    // then: the scene is refused
    let bytes = heard_by_ada.recv().await.expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone())
    );
}

#[tokio::test]
async fn a_reader_joins_asks_what_is_there_and_hears_an_author_s_edits() {
    // given: a scene in a project with members
    let guarded = a_guarded_scene().await;

    // when: an author and a reader join it
    let (mut ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    // then: the reader is greeted
    assert!(
        matches!(
            everything(&mut heard_by_bo).await.as_slice(),
            [Message::WhatDoYouHave(_)]
        ),
        "the reader is greeted as an author is"
    );

    // when: the author edits
    send(
        &mut ada,
        &guarded.scene,
        Message::JustHappened(a_paragraph("The loom stood silent.")),
    )
    .await;
    // and: the reader asks what is there
    send(&mut bo, &guarded.scene, Message::WhatDoYouHave(vec![0])).await;

    // then: the reader hears the edit and what it missed
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
    // and: the edit is stored
    assert_eq!(guarded.text().await, "The loom stood silent.");
}

#[tokio::test]
async fn what_a_reader_sends_to_change_the_text_goes_nowhere() {
    // given: an author and a reader in a scene
    let guarded = a_guarded_scene().await;
    let (_ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;

    // when: the reader sends changes to the text
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

    // then: nobody hears anything
    assert!(everything(&mut heard_by_ada).await.is_empty());
    assert!(everything(&mut heard_by_bo).await.is_empty());
    // and: the text is unchanged
    assert_eq!(guarded.text().await, "");
}

#[tokio::test]
async fn a_reader_is_seen_by_the_others_and_asks_who_is_there() {
    // given: an author and a reader in a scene
    let guarded = a_guarded_scene().await;
    let (_ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;

    // when: the reader tells where it is and asks who is there
    send(&mut bo, &guarded.scene, Message::Awareness(vec![1, 2, 3])).await;
    send(&mut bo, &guarded.scene, Message::WhoIsHere).await;

    // then: the author hears both
    assert_eq!(
        everything(&mut heard_by_ada).await,
        vec![Message::Awareness(vec![1, 2, 3]), Message::WhoIsHere],
        "where a reader's cursor is shows for everyone in the scene"
    );
}

async fn asking_again(connection: &mut Connection) {
    let scene = tokio::time::timeout(Wait::from_secs(1), connection.access_changed())
        .await
        .expect("a change to the members is heard");
    connection.ask_again(scene).await;
}

#[tokio::test]
async fn a_member_removed_is_refused_the_scene_they_had_joined_and_heard_no_more() {
    // given: an owner and a member in a scene
    let guarded = a_guarded_scene().await;
    let (mut ada, mut heard_by_ada) = guarded.joined_as("account_ada").await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_ada).await;
    everything(&mut heard_by_bo).await;

    // when: the member is removed and both connections ask again
    guarded.roles.change("account_bo", &[]);
    asking_again(&mut bo).await;
    asking_again(&mut ada).await;

    // then: the removed member is refused the scene
    let bytes = tokio::time::timeout(Wait::from_secs(1), heard_by_bo.recv())
        .await
        .expect("told while it is fresh")
        .expect("an answer");
    assert_eq!(
        Envelope::decode(&bytes).expect("a whole envelope"),
        Envelope::Refused(guarded.scene.clone())
    );
    // when: each sends to the scene
    send(&mut bo, &guarded.scene, Message::Awareness(vec![1, 2, 3])).await;
    send(
        &mut ada,
        &guarded.scene,
        Message::JustHappened(a_paragraph("Not for Bo.")),
    )
    .await;
    // then: neither hears the other
    assert!(everything(&mut heard_by_ada).await.is_empty());
    assert!(
        everything(&mut heard_by_bo).await.is_empty(),
        "nothing more of the scene reaches whoever left it"
    );
    // and: the owner's edit is stored
    assert_eq!(guarded.text().await, "Not for Bo.", "the owner writes on");
}

#[tokio::test]
async fn a_reader_made_an_author_writes_and_an_author_made_a_reader_no_longer_does() {
    // given: a reader in a scene
    let guarded = a_guarded_scene().await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_bo).await;

    // when: the reader is made an author
    guarded.roles.change("account_bo", &[Act::Read, Act::Write]);
    asking_again(&mut bo).await;
    // and: edits the scene
    send(
        &mut bo,
        &guarded.scene,
        Message::JustHappened(a_paragraph("Bo writes now.")),
    )
    .await;
    // then: the edit is stored
    assert_eq!(guarded.text().await, "Bo writes now.");

    // when: the author is made a reader again
    guarded.roles.change("account_bo", &READER);
    asking_again(&mut bo).await;
    // and: edits the scene
    send(
        &mut bo,
        &guarded.scene,
        Message::JustHappened(a_paragraph("And no more.")),
    )
    .await;
    // then: the edit is not stored
    assert_eq!(guarded.text().await, "Bo writes now.");
}

#[tokio::test]
async fn when_who_may_stay_cannot_be_asked_the_scene_is_refused() {
    // given: a member in a scene
    let guarded = a_guarded_scene().await;
    let (mut bo, mut heard_by_bo) = guarded.joined_as("account_bo").await;
    everything(&mut heard_by_bo).await;

    // when: the members change while who may stay cannot be asked
    *guarded.roles.down.lock().expect("never poisoned") = true;
    guarded.roles.access_changed(PROJECT);
    asking_again(&mut bo).await;

    // then: the scene is refused
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
    // given: a member joined on the socket
    let guarded = a_guarded_scene().await;
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

    // when: the member is removed
    guarded.roles.change("account_bo", &[]);

    // then: the socket refuses the scene
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
    // given: a socket opened in a session
    let guarded = a_guarded_scene().await;
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

    // when: the session ends
    signing.0.notify_one();

    // then: the socket is closed as ended by its session
    assert!(matches!(
        tokio::time::timeout(Wait::from_secs(1), socket.receive_message()).await,
        Ok(WsMessage::Close(Some(frame))) if u16::from(frame.code) == SESSION_ENDED
    ));
}

#[tokio::test]
async fn the_socket_stops_taking_edits_when_the_server_closes_and_is_waited_for_until_it_has() {
    // given: an open socket
    let guarded = a_guarded_scene().await;
    let shutdown = ending::Shutdown::new();
    let server = TestServer::builder().http_transport().build(
        scenes_sync::router(guarded.live.clone())
            .layer(Extension(account("account_bo")))
            .layer(Extension(shutdown.clone())),
    );
    let mut socket = server
        .get_websocket("/scenes/sync")
        .await
        .into_websocket()
        .await;
    // and: the server not drained while it is open
    let open = tokio::time::timeout(Wait::from_millis(50), shutdown.drained()).await;

    // when: the server closes
    shutdown.close();

    // then: the open socket was waited for
    assert!(open.is_err(), "an open socket is waited for");
    // and: the socket is closed as the server closing
    assert!(matches!(
        tokio::time::timeout(Wait::from_secs(1), socket.receive_message()).await,
        Ok(WsMessage::Close(Some(frame))) if u16::from(frame.code) == SERVER_CLOSING
    ));
    // and: the server is drained
    assert!(
        tokio::time::timeout(Wait::from_secs(1), shutdown.drained())
            .await
            .is_ok(),
        "told the server is closing, it takes no more edits and lets the scenes be settled"
    );
}

#[tokio::test]
async fn a_message_larger_than_any_scene_sent_ends_the_socket() {
    // given: the owner joined to a scene on the socket
    let guarded = a_guarded_scene().await;
    let server = TestServer::builder()
        .http_transport()
        .build(scenes_sync::router(guarded.live.clone()).layer(Extension(account("account_ada"))));
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
    tokio::time::timeout(Wait::from_secs(1), socket.receive_bytes())
        .await
        .expect("greeted while it is fresh");

    // when: a message larger than any scene is sent
    let sending = tokio::spawn(async move {
        socket
            .send_message(WsMessage::Binary(vec![0; LARGEST_SENT + 1].into()))
            .await;
    });
    let _ended_while_sending = tokio::time::timeout(Wait::from_secs(1), sending).await;
    tokio::time::sleep(Wait::from_millis(100)).await;

    // then: the socket is ended rather than the message kept, so the scene it had joined is left and let go
    guarded.live.sweep(Instant::now()).await;
    assert!(
        !guarded
            .live
            .is_live(guarded.scene.parse().expect("a scene id")),
        "nobody is left in the scene"
    );
}
