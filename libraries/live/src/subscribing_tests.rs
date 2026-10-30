use std::collections::BTreeSet;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use acting::{Ending, SessionEnd};
use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch};
use axum::Extension;
use axum_test::TestServer;
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, FrameDTO, SESSION_ENDED, ToldDTO, ended};
use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::testing::Subscriber;
use crate::{
    About, Broadcaster, Gate, GatedTopic, Refused, Sieve, Subscribed, Topic, Topics, live_socket,
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
    changes: AccessChanges,
}

#[async_trait]
impl Gate for Losing {
    async fn admits(
        &self,
        _agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
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
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "mine").await;

    live.broadcaster
        .tell("theirs".to_owned(), frame("theirs", 1));
    live.broadcaster.tell("mine".to_owned(), frame("mine", 2));

    assert_eq!(socket.told().await, framed("a", "mine", 2));
    socket.nothing().await;
}

#[tokio::test]
async fn one_socket_holds_many_subscriptions_and_one_unsubscribed_hears_no_more() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    subscribe(&mut socket, "b", "open", "outline").await;

    live.broadcaster.tell("board".to_owned(), frame("board", 1));
    assert_eq!(socket.told().await, framed("a", "board", 1));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 1));
    assert_eq!(socket.told().await, framed("b", "outline", 1));

    socket.unsubscribe("a").await;
    live.broadcaster.tell("board".to_owned(), frame("board", 2));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 2));

    assert_eq!(socket.told().await, framed("b", "outline", 2));
    socket.nothing().await;
}

#[tokio::test]
async fn subscribing_again_under_one_id_replaces_what_it_followed() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;

    subscribe(&mut socket, "a", "open", "outline").await;
    live.broadcaster.tell("board".to_owned(), frame("board", 1));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 1));

    assert_eq!(socket.told().await, framed("a", "outline", 1));
    socket.nothing().await;
}

#[tokio::test]
async fn a_subscription_nobody_can_follow_ends_saying_why_and_the_socket_stays() {
    let live = served();
    let mut socket = opened(&live).await;

    for (id, topic, why) in [
        ("a", "nobody-registered-this", ended::UNKNOWN_TOPIC),
        ("b", "not-there", ended::NOT_THERE),
        ("c", "failing", ended::FAILED),
    ] {
        subscribe(&mut socket, id, topic, "mine").await;
        assert_eq!(socket.told().await, ended_as(id, why));
    }

    subscribe(&mut socket, "d", "open", "mine").await;
    live.broadcaster.tell("mine".to_owned(), frame("mine", 1));
    assert_eq!(socket.told().await, framed("d", "mine", 1));
}

#[tokio::test]
async fn a_subscription_standing_at_a_version_is_told_it_first() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "from-seven", "mine").await;

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
    live.broadcaster.tell("mine".to_owned(), frame("mine", 8));
    assert_eq!(socket.told().await, framed("a", "mine", 8));
}

#[tokio::test]
async fn losing_access_ends_that_subscription_alone_and_keeping_it_is_told() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "watched", "lost").await;
    subscribe(&mut socket, "b", "watched", "kept").await;

    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("lost".to_owned());
    live.changes.tell("lost");
    assert_eq!(socket.told().await, ended_as("a", ended::ACCESS_LOST));

    live.changes.tell("kept");
    assert_eq!(
        socket.told().await,
        ToldDTO::Notice {
            id: "b".to_owned(),
            notice: ACCESS_CHANGED.to_owned(),
        },
        "its role may be another now, so its client asks again"
    );
    live.broadcaster.tell("lost".to_owned(), frame("lost", 1));
    live.broadcaster.tell("kept".to_owned(), frame("kept", 1));
    assert_eq!(socket.told().await, framed("b", "kept", 1));
    socket.nothing().await;
}

#[tokio::test]
async fn a_subscription_falling_behind_ends_so_and_the_others_follow_on() {
    let live = served_with(Broadcaster::holding(2));
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "busy").await;

    for version in 1..=5 {
        live.broadcaster
            .tell("busy".to_owned(), frame("busy", version));
    }
    assert_eq!(socket.told().await, ended_as("a", ended::BEHIND));

    subscribe(&mut socket, "b", "open", "busy").await;
    live.broadcaster.tell("busy".to_owned(), frame("busy", 6));
    assert_eq!(
        socket.told().await,
        framed("b", "busy", 6),
        "subscribed again, it follows from where it is"
    );
}

#[tokio::test]
async fn a_sifted_subscription_hears_what_its_sieve_lets_through_and_ends_when_it_cannot_tell() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "sifted", "mixed").await;

    live.broadcaster
        .tell("mixed".to_owned(), frame("held back", 1));
    live.broadcaster.tell("mixed".to_owned(), frame("kept", 2));
    assert_eq!(socket.told().await, framed("a", "kept", 2));

    live.broadcaster
        .tell("mixed".to_owned(), frame("broken", 3));
    assert_eq!(socket.told().await, ended_as("a", ended::FAILED));
}

#[tokio::test]
async fn closing_the_socket_ends_every_subscription_it_held() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    subscribe(&mut socket, "b", "open", "outline").await;
    assert_eq!(live.broadcaster.listener_count(), 2);

    socket.close().await;

    assert_eq!(
        live.broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}

#[tokio::test]
async fn a_gated_topic_tells_whom_its_gate_lets_in_and_ends_when_access_is_lost() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "gated", "mine").await;

    live.broadcaster.tell("mine".to_owned(), frame("mine", 1));
    assert_eq!(socket.told().await, framed("a", "mine", 1));

    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("mine".to_owned());
    live.changes.tell("mine");
    assert_eq!(socket.told().await, ended_as("a", ended::ACCESS_LOST));
}

#[tokio::test]
async fn a_gated_topic_turns_away_whom_its_gate_does_and_fails_when_it_cannot_tell() {
    let live = served();
    live.gate
        .lost
        .lock()
        .expect("the gate is not poisoned")
        .insert("theirs".to_owned());
    let mut socket = opened(&live).await;

    subscribe(&mut socket, "a", "gated", "theirs").await;
    assert_eq!(socket.told().await, ended_as("a", ended::NOT_THERE));
    subscribe(&mut socket, "b", "gated", "broken").await;
    assert_eq!(socket.told().await, ended_as("b", ended::FAILED));

    socket.subscribe("c", "gated", &[]).await;
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
    let broadcaster = Broadcaster::new();
    let signing = Arc::new(Signing(Notify::new()));
    let server = TestServer::builder().http_transport().build(
        live_socket(Topics::of(vec![Arc::new(Open(broadcaster.clone()))]))
            .layer(Extension(Agent::Local))
            .layer(Extension(SessionEnd::of(signing.clone()))),
    );
    let mut socket = Subscriber::on(&server).await;
    subscribe(&mut socket, "a", "open", "board").await;

    signing.0.notify_one();

    assert_eq!(socket.closed_as().await, Some(SESSION_ENDED));
    settled().await;
    assert_eq!(
        broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}
