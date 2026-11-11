use std::collections::BTreeSet;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch};
use axum::Extension;
use axum_test::TestServer;
use ending::{Ending, SessionEnd};
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, FrameDTO, SERVER_CLOSING, SESSION_ENDED, ToldDTO, ended};
use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::testing::Subscriber;
use crate::{
    About, Broadcaster, Gate, GatedTopic, LARGEST_ASKED, Refused, Sieve, Subscribed, Topic, Topics,
    live_socket,
};

fn of(about: &About) -> String {
    about.get("of").cloned().unwrap_or_default()
}

struct Open(Broadcaster<String>);

#[async_trait]
impl Topic for Open {
    fn named(&self) -> &'static str {
        "open"
    }

    async fn subscribe(&self, _agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        Ok(Subscribed::hearing(self.0.hear(of(about))))
    }
}

struct Refusing(&'static str, Refused);

#[async_trait]
impl Topic for Refusing {
    fn named(&self) -> &'static str {
        self.0
    }

    async fn subscribe(&self, _agent: &Agent, _about: &About) -> Result<Subscribed, Refused> {
        Err(self.1)
    }
}

struct FromSeven(Broadcaster<String>);

#[async_trait]
impl Topic for FromSeven {
    fn named(&self) -> &'static str {
        "from-seven"
    }

    async fn subscribe(&self, _agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        Ok(Subscribed::hearing(self.0.hear(of(about))).standing_at(7))
    }
}

struct Losing {
    lost: Mutex<BTreeSet<String>>,
    slow: Mutex<BTreeSet<String>>,
    changes: AccessChanges,
}

#[async_trait]
impl Gate for Losing {
    async fn admits(
        &self,
        _agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let slow = self
            .slow
            .lock()
            .expect("the gate is not poisoned")
            .contains(about);
        if slow {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        match about {
            "broken" => Err("the gate cannot tell".into()),
            about => Ok(!self
                .lost
                .lock()
                .expect("the gate is not poisoned")
                .contains(about)),
        }
    }

    async fn watch(&self, about: &str) -> AccessWatch {
        self.changes.watch(about)
    }
}

struct Watched {
    broadcaster: Broadcaster<String>,
    gate: Arc<Losing>,
}

#[async_trait]
impl Topic for Watched {
    fn named(&self) -> &'static str {
        "watched"
    }

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        let about = of(about);
        let admission = crate::admitted(self.gate.clone(), agent, &about).await?;

        Ok(Subscribed::hearing(self.broadcaster.hear(about)).while_admitted(admission))
    }
}

struct OnlyKept;

#[async_trait]
impl Sieve for OnlyKept {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match frame.of.as_deref() {
            Some("broken") => Err("the sieve cannot tell".into()),
            kept => Ok(kept == Some("kept")),
        }
    }
}

struct Sifted(Broadcaster<String>);

#[async_trait]
impl Topic for Sifted {
    fn named(&self) -> &'static str {
        "sifted"
    }

    async fn subscribe(&self, _agent: &Agent, about: &About) -> Result<Subscribed, Refused> {
        Ok(Subscribed::hearing(self.0.hear(of(about))).sifted(Arc::new(OnlyKept)))
    }
}

struct Live {
    broadcaster: Broadcaster<String>,
    changes: AccessChanges,
    gate: Arc<Losing>,
    server: TestServer,
}

fn served_with(broadcaster: Broadcaster<String>) -> Live {
    let changes = AccessChanges::new();
    let gate = Arc::new(Losing {
        lost: Mutex::new(BTreeSet::new()),
        slow: Mutex::new(BTreeSet::new()),
        changes: changes.clone(),
    });
    let topics = Topics::of(vec![
        Arc::new(Open(broadcaster.clone())),
        Arc::new(FromSeven(broadcaster.clone())),
        Arc::new(Sifted(broadcaster.clone())),
        Arc::new(Watched {
            broadcaster: broadcaster.clone(),
            gate: gate.clone(),
        }),
        Arc::new(Refusing("not-there", Refused::NotThere)),
        Arc::new(GatedTopic::new(
            "gated",
            broadcaster.clone(),
            |about| about.get("of").cloned(),
            gate.clone(),
        )),
        Arc::new(Refusing("failing", Refused::Failed)),
    ]);

    Live {
        broadcaster,
        changes,
        gate,
        server: TestServer::builder()
            .http_transport()
            .build(live_socket(topics).layer(Extension(Agent::Local))),
    }
}

fn served() -> Live {
    served_with(Broadcaster::new())
}

async fn opened(live: &Live) -> Subscriber {
    Subscriber::on(&live.server).await
}

async fn subscribe(socket: &mut Subscriber, id: &str, topic: &str, about: &str) {
    socket.subscribe(id, topic, &[("of", about)]).await;
}

async fn settled() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

fn frame(of: &str, version: u64) -> String {
    json!({ "of": of, "version": version, "event": null }).to_string()
}

fn framed(id: &str, of: &str, version: u64) -> ToldDTO {
    ToldDTO::Frame {
        id: id.to_owned(),
        frame: FrameDTO {
            of: Some(of.to_owned()),
            version,
            at: None,
            event: Value::Null,
        },
    }
}

fn ended_as(id: &str, why: &str) -> ToldDTO {
    ToldDTO::Ended {
        id: id.to_owned(),
        why: why.to_owned(),
    }
}

#[tokio::test]
async fn a_subscription_hears_its_own_frames_under_its_id_and_no_others() {
    // given: a subscription on an open socket
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "mine").await;

    // when: frames are told on another channel and on its own
    live.broadcaster
        .tell("theirs".to_owned(), frame("theirs", 1));
    live.broadcaster.tell("mine".to_owned(), frame("mine", 2));

    // then: it hears its own under its id
    assert_eq!(socket.told().await, framed("a", "mine", 2));
    // and: nothing else
    socket.nothing().await;
}

#[tokio::test]
async fn one_socket_holds_many_subscriptions_and_one_unsubscribed_hears_no_more() {
    // given: two subscriptions on one socket
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    subscribe(&mut socket, "b", "open", "outline").await;

    // when: a frame is told for the first
    live.broadcaster.tell("board".to_owned(), frame("board", 1));
    // then: the first hears it
    assert_eq!(socket.told().await, framed("a", "board", 1));
    // when: a frame is told for the second
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 1));
    // then: the second hears it
    assert_eq!(socket.told().await, framed("b", "outline", 1));

    // when: the first is unsubscribed and frames are told for both
    socket.unsubscribe("a").await;
    live.broadcaster.tell("board".to_owned(), frame("board", 2));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 2));

    // then: only the second hears
    assert_eq!(socket.told().await, framed("b", "outline", 2));
    socket.nothing().await;
}

#[tokio::test]
async fn subscribing_again_under_one_id_replaces_what_it_followed() {
    // given: a subscription following the board
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;

    // when: it subscribes again under its id to the outline
    subscribe(&mut socket, "a", "open", "outline").await;
    // and: frames are told for both
    live.broadcaster.tell("board".to_owned(), frame("board", 1));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 1));

    // then: it hears only the outline
    assert_eq!(socket.told().await, framed("a", "outline", 1));
    socket.nothing().await;
}

#[tokio::test]
async fn a_subscription_nobody_can_follow_ends_saying_why_and_the_socket_stays() {
    // given: an open socket
    let live = served();
    let mut socket = opened(&live).await;

    // when: it subscribes to each of what nobody can follow
    for (id, topic, why) in [
        ("a", "nobody-registered-this", ended::UNKNOWN_TOPIC),
        ("b", "not-there", ended::NOT_THERE),
        ("c", "failing", ended::FAILED),
    ] {
        subscribe(&mut socket, id, topic, "mine").await;
        // then: that subscription ends saying why
        assert_eq!(socket.told().await, ended_as(id, why));
    }

    // when: it then subscribes to what can be followed
    subscribe(&mut socket, "d", "open", "mine").await;
    live.broadcaster.tell("mine".to_owned(), frame("mine", 1));
    // then: it still hears
    assert_eq!(socket.told().await, framed("d", "mine", 1));
}

#[tokio::test]
async fn a_subscription_standing_at_a_version_is_told_it_first() {
    // when: a subscription standing at a version is made
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "from-seven", "mine").await;

    // then: it is told that version first
    assert_eq!(
        socket.told().await,
        ToldDTO::Frame {
            id: "a".to_owned(),
            frame: FrameDTO {
                of: None,
                version: 7,
                at: None,
                event: Value::Null,
            },
        }
    );
    // when: a newer frame is told
    live.broadcaster.tell("mine".to_owned(), frame("mine", 8));
    // then: it hears it after
    assert_eq!(socket.told().await, framed("a", "mine", 8));
}

#[tokio::test]
async fn losing_access_ends_that_subscription_alone_and_keeping_it_is_told() {
    // given: two watched subscriptions on one socket
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "watched", "lost").await;
    subscribe(&mut socket, "b", "watched", "kept").await;

    // when: access to the first is lost
    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("lost".to_owned());
    live.changes.tell("lost");
    // then: the first ends as access lost
    assert_eq!(socket.told().await, ended_as("a", ended::ACCESS_LOST));

    // when: access to the second changes but is kept
    live.changes.tell("kept");
    // then: the second is told its access changed
    assert_eq!(
        socket.told().await,
        ToldDTO::Notice {
            id: "b".to_owned(),
            notice: ACCESS_CHANGED.to_owned(),
        },
        "its role may be another now, so its client asks again"
    );
    // when: frames are told for both
    live.broadcaster.tell("lost".to_owned(), frame("lost", 1));
    live.broadcaster.tell("kept".to_owned(), frame("kept", 1));
    // then: only the kept one hears
    assert_eq!(socket.told().await, framed("b", "kept", 1));
    socket.nothing().await;
}

#[tokio::test]
async fn access_lost_while_a_frame_arrives_still_ends_the_subscription() {
    // given: a watched subscription
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "watched", "removed").await;
    settled().await;

    // when: access is lost while its gate is slow to tell
    live.gate
        .slow
        .lock()
        .expect("the gate is not poisoned")
        .insert("removed".to_owned());
    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("removed".to_owned());
    live.changes.tell("removed");
    settled().await;
    // and: a frame arrives meanwhile
    live.broadcaster
        .tell("removed".to_owned(), frame("removed", 1));

    // then: the subscription ends as access lost
    assert_eq!(
        socket.told().await,
        ended_as("a", ended::ACCESS_LOST),
        "a removal comes mostly with a frame on the same channel, and the frame must not win"
    );
}

#[tokio::test]
async fn a_subscription_falling_behind_ends_so_and_the_others_follow_on() {
    // given: a subscription on a broadcaster holding two frames
    let live = served_with(Broadcaster::holding(2));
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "busy").await;

    // when: more are told than it can keep up with
    for version in 1..=5 {
        live.broadcaster
            .tell("busy".to_owned(), frame("busy", version));
    }
    // then: it ends as behind
    assert_eq!(socket.told().await, ended_as("a", ended::BEHIND));

    // when: another subscribes and a frame is told
    subscribe(&mut socket, "b", "open", "busy").await;
    live.broadcaster.tell("busy".to_owned(), frame("busy", 6));
    // then: the other hears it
    assert_eq!(
        socket.told().await,
        framed("b", "busy", 6),
        "subscribed again, it follows from where it is"
    );
}

#[tokio::test]
async fn a_sifted_subscription_hears_what_its_sieve_lets_through_and_ends_when_it_cannot_tell() {
    // given: a sifted subscription
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "sifted", "mixed").await;

    // when: a frame its sieve holds back and one it lets through are told
    live.broadcaster
        .tell("mixed".to_owned(), frame("held back", 1));
    live.broadcaster.tell("mixed".to_owned(), frame("kept", 2));
    // then: it hears only the one let through
    assert_eq!(socket.told().await, framed("a", "kept", 2));

    // when: a frame its sieve cannot tell about is told
    live.broadcaster
        .tell("mixed".to_owned(), frame("broken", 3));
    // then: it ends as failed
    assert_eq!(socket.told().await, ended_as("a", ended::FAILED));
}

#[tokio::test]
async fn closing_the_socket_ends_every_subscription_it_held() {
    // given: a socket holding two subscriptions
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    subscribe(&mut socket, "b", "open", "outline").await;
    assert_eq!(live.broadcaster.listener_count(), 2);

    // when: it is closed
    socket.close().await;

    // then: nobody hears for it any more
    assert_eq!(
        live.broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}

#[tokio::test]
async fn a_gated_topic_tells_whom_its_gate_lets_in_and_ends_when_access_is_lost() {
    // given: a subscription to a gated topic its gate lets in
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "gated", "mine").await;

    // when: a frame is told
    live.broadcaster.tell("mine".to_owned(), frame("mine", 1));
    // then: it hears it
    assert_eq!(socket.told().await, framed("a", "mine", 1));

    // when: its access is lost
    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("mine".to_owned());
    live.changes.tell("mine");
    // then: it ends as access lost
    assert_eq!(socket.told().await, ended_as("a", ended::ACCESS_LOST));
}

#[tokio::test]
async fn a_gated_topic_turns_away_whom_its_gate_does_and_fails_when_it_cannot_tell() {
    // given: a gate that turns one away
    let live = served();
    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("theirs".to_owned());
    // and: an open socket
    let mut socket = opened(&live).await;

    // when: it subscribes to what the gate turns away
    subscribe(&mut socket, "a", "gated", "theirs").await;
    // then: that ends as not there
    assert_eq!(socket.told().await, ended_as("a", ended::NOT_THERE));
    // when: it subscribes to what the gate cannot tell about
    subscribe(&mut socket, "b", "gated", "broken").await;
    // then: that ends as failed
    assert_eq!(socket.told().await, ended_as("b", ended::FAILED));

    // when: it subscribes about nothing
    socket.subscribe("c", "gated", &[]).await;
    // then: that ends as not there
    assert_eq!(
        socket.told().await,
        ended_as("c", ended::NOT_THERE),
        "asking about nothing finds nothing there"
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
async fn a_socket_whose_session_ends_is_closed_as_such_and_its_subscriptions_with_it() {
    // given: a subscription on a socket of a session
    let broadcaster = Broadcaster::new();
    let signing = Arc::new(Signing(Notify::new()));
    let server = TestServer::builder().http_transport().build(
        live_socket(Topics::of(vec![Arc::new(Open(broadcaster.clone()))]))
            .layer(Extension(Agent::Local))
            .layer(Extension(SessionEnd::of(signing.clone()))),
    );
    let mut socket = Subscriber::on(&server).await;
    subscribe(&mut socket, "a", "open", "board").await;

    // when: the session ends
    signing.0.notify_one();

    // then: the socket is closed as session ended
    assert_eq!(socket.closed_as().await, Some(SESSION_ENDED));
    settled().await;
    // and: nobody hears for it any more
    assert_eq!(
        broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}

#[tokio::test]
async fn a_subscription_is_told_followed_before_anything_it_hears() {
    // given: an open socket
    let live = served();
    let mut socket = opened(&live).await;

    // when: it subscribes to a topic standing at a version
    subscribe(&mut socket, "s1", "from-seven", "loom").await;

    // then: it is told followed first
    socket.following("s1").await;
    // and: the version after
    assert_eq!(
        serde_json::to_value(socket.told().await).expect("plain data")["frame"]["version"],
        7
    );
}

#[tokio::test]
async fn a_change_told_while_the_gate_is_asked_reaches_the_follower() {
    // given: a gate slow to tell
    let live = served();
    live.gate
        .slow
        .lock()
        .expect("the gate is not poisoned")
        .insert("loom".to_owned());
    // and: an open socket
    let mut socket = opened(&live).await;

    // when: it subscribes through that gate
    subscribe(&mut socket, "s1", "gated", "loom").await;
    // and: a change is told while the gate is asked
    live.broadcaster.tell("loom".to_owned(), frame("loom", 3));

    // then: it is told followed
    socket.following("s1").await;
    // and: it hears the change
    assert_eq!(
        socket.told().await,
        framed("s1", "loom", 3),
        "the client reads once told it is followed, so a change before then must still be heard"
    );
}

#[tokio::test]
async fn a_socket_is_closed_when_the_server_closes_and_is_waited_for_until_it_has() {
    // given: a subscription on an open socket
    let broadcaster = Broadcaster::new();
    let shutdown = ending::Shutdown::new();
    let server = TestServer::builder().http_transport().build(
        live_socket(Topics::of(vec![Arc::new(Open(broadcaster.clone()))]))
            .layer(Extension(Agent::Local))
            .layer(Extension(shutdown.clone())),
    );
    let mut socket = Subscriber::on(&server).await;
    subscribe(&mut socket, "a", "open", "board").await;
    // and: the server waiting on it
    let open = tokio::time::timeout(Duration::from_millis(50), shutdown.drained()).await;

    // when: the server closes
    shutdown.close();

    // then: it was waited for
    assert!(open.is_err(), "an open socket is waited for");
    // and: it is closed as server closing
    assert_eq!(socket.closed_as().await, Some(SERVER_CLOSING));
    // and: nothing is left to wait for
    assert!(
        tokio::time::timeout(Duration::from_secs(1), shutdown.drained())
            .await
            .is_ok(),
        "once closed, nothing is left to wait for"
    );
    settled().await;
    // and: nobody hears for it any more
    assert_eq!(
        broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}

#[tokio::test]
async fn a_message_larger_than_anything_asked_ends_the_socket() {
    // given: a subscription on an open socket
    let broadcaster = Broadcaster::new();
    let live = served_with(broadcaster.clone());
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    socket.following("a").await;

    // when: a message larger than any a client asks is sent
    socket.send_text("x".repeat(LARGEST_ASKED + 1)).await;
    settled().await;

    // then: the socket is ended rather than the message kept, its subscriptions with it
    assert_eq!(
        broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}
