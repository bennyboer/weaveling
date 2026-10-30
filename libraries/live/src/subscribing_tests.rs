use std::collections::BTreeSet;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use authorizing::AccessChanges;
use axum::Extension;
use axum_test::{TestServer, TestWebSocket};
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, AskedDTO, FrameDTO, ToldDTO, ended};
use serde_json::{Value, json};

use crate::following::Asked;
use crate::socket::{Admission, Sieve};
use crate::{Broadcaster, Gate, Refused, Subscribed, Topic, Topics, live_socket};

fn of(about: &Asked) -> String {
    about.get("of").cloned().unwrap_or_default()
}

struct Open(Broadcaster<String>);

#[async_trait]
impl Topic for Open {
    fn named(&self) -> &'static str {
        "open"
    }

    async fn subscribe(&self, _agent: &Agent, about: &Asked) -> Result<Subscribed, Refused> {
        Ok(Subscribed::hearing(self.0.hear(of(about))))
    }
}

struct Refusing(&'static str, Refused);

#[async_trait]
impl Topic for Refusing {
    fn named(&self) -> &'static str {
        self.0
    }

    async fn subscribe(&self, _agent: &Agent, _about: &Asked) -> Result<Subscribed, Refused> {
        Err(self.1)
    }
}

struct FromSeven(Broadcaster<String>);

#[async_trait]
impl Topic for FromSeven {
    fn named(&self) -> &'static str {
        "from-seven"
    }

    async fn subscribe(&self, _agent: &Agent, about: &Asked) -> Result<Subscribed, Refused> {
        Ok(Subscribed::hearing(self.0.hear(of(about))).standing_at(7))
    }
}

struct Losing {
    lost: Mutex<BTreeSet<String>>,
}

#[async_trait]
impl Gate for Losing {
    async fn admits(
        &self,
        _agent: &Agent,
        about: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(!self
            .lost
            .lock()
            .expect("the gate is not poisoned")
            .contains(about))
    }
}

struct Watched {
    broadcaster: Broadcaster<String>,
    changes: AccessChanges,
    gate: Arc<Losing>,
}

#[async_trait]
impl Topic for Watched {
    fn named(&self) -> &'static str {
        "watched"
    }

    async fn subscribe(&self, agent: &Agent, about: &Asked) -> Result<Subscribed, Refused> {
        let about = of(about);

        Ok(
            Subscribed::hearing(self.broadcaster.hear(about.clone())).while_admitted(
                Admission::new(
                    self.changes.watch(&about),
                    self.gate.clone(),
                    agent.clone(),
                    about,
                ),
            ),
        )
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

    async fn subscribe(&self, _agent: &Agent, about: &Asked) -> Result<Subscribed, Refused> {
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
    });
    let topics = Topics::of(vec![
        Arc::new(Open(broadcaster.clone())),
        Arc::new(FromSeven(broadcaster.clone())),
        Arc::new(Sifted(broadcaster.clone())),
        Arc::new(Watched {
            broadcaster: broadcaster.clone(),
            changes: changes.clone(),
            gate: gate.clone(),
        }),
        Arc::new(Refusing("not-there", Refused::NotThere)),
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

async fn opened(live: &Live) -> TestWebSocket {
    live.server
        .get_websocket("/live")
        .await
        .into_websocket()
        .await
}

async fn subscribe(socket: &mut TestWebSocket, id: &str, topic: &str, about: &str) {
    socket
        .send_json(&AskedDTO::Subscribe {
            id: id.to_owned(),
            topic: topic.to_owned(),
            about: [("of".to_owned(), about.to_owned())].into(),
        })
        .await;
    settled().await;
}

async fn settled() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

async fn told(socket: &mut TestWebSocket) -> ToldDTO {
    tokio::time::timeout(Duration::from_secs(1), socket.receive_json::<ToldDTO>())
        .await
        .expect("told while fresh")
}

async fn nothing_more(socket: &mut TestWebSocket) {
    assert!(
        tokio::time::timeout(Duration::from_millis(150), socket.receive_text())
            .await
            .is_err(),
        "nothing else is told"
    );
}

fn frame(of: &str, version: u64) -> String {
    json!({ "of": of, "version": version, "event": null }).to_string()
}

fn framed(id: &str, of: &str, version: u64) -> ToldDTO {
    ToldDTO::Frame {
        id: id.to_owned(),
        frame: serde_json::from_str::<Value>(&frame(of, version)).expect("a frame"),
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

    assert_eq!(told(&mut socket).await, framed("a", "mine", 2));
    nothing_more(&mut socket).await;
}

#[tokio::test]
async fn one_socket_holds_many_subscriptions_and_one_unsubscribed_hears_no_more() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    subscribe(&mut socket, "b", "open", "outline").await;

    live.broadcaster.tell("board".to_owned(), frame("board", 1));
    assert_eq!(told(&mut socket).await, framed("a", "board", 1));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 1));
    assert_eq!(told(&mut socket).await, framed("b", "outline", 1));

    socket
        .send_json(&AskedDTO::Unsubscribe { id: "a".to_owned() })
        .await;
    settled().await;
    live.broadcaster.tell("board".to_owned(), frame("board", 2));
    live.broadcaster
        .tell("outline".to_owned(), frame("outline", 2));

    assert_eq!(told(&mut socket).await, framed("b", "outline", 2));
    nothing_more(&mut socket).await;
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

    assert_eq!(told(&mut socket).await, framed("a", "outline", 1));
    nothing_more(&mut socket).await;
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
        assert_eq!(told(&mut socket).await, ended_as(id, why));
    }

    subscribe(&mut socket, "d", "open", "mine").await;
    live.broadcaster.tell("mine".to_owned(), frame("mine", 1));
    assert_eq!(told(&mut socket).await, framed("d", "mine", 1));
}

#[tokio::test]
async fn a_subscription_standing_at_a_version_is_told_it_first() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "from-seven", "mine").await;

    assert_eq!(
        told(&mut socket).await,
        ToldDTO::Frame {
            id: "a".to_owned(),
            frame: json!({ "version": 7 }),
        }
    );
    live.broadcaster.tell("mine".to_owned(), frame("mine", 8));
    assert_eq!(told(&mut socket).await, framed("a", "mine", 8));
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
    assert_eq!(told(&mut socket).await, ended_as("a", ended::ACCESS_LOST));

    live.changes.tell("kept");
    assert_eq!(
        told(&mut socket).await,
        ToldDTO::Notice {
            id: "b".to_owned(),
            notice: ACCESS_CHANGED.to_owned(),
        },
        "its role may be another now, so its client asks again"
    );
    live.broadcaster.tell("lost".to_owned(), frame("lost", 1));
    live.broadcaster.tell("kept".to_owned(), frame("kept", 1));
    assert_eq!(told(&mut socket).await, framed("b", "kept", 1));
    nothing_more(&mut socket).await;
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
    assert_eq!(told(&mut socket).await, ended_as("a", ended::BEHIND));

    subscribe(&mut socket, "b", "open", "busy").await;
    live.broadcaster.tell("busy".to_owned(), frame("busy", 6));
    assert_eq!(
        told(&mut socket).await,
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
    assert_eq!(told(&mut socket).await, framed("a", "kept", 2));

    live.broadcaster
        .tell("mixed".to_owned(), frame("broken", 3));
    assert_eq!(told(&mut socket).await, ended_as("a", ended::FAILED));
}

#[tokio::test]
async fn closing_the_socket_ends_every_subscription_it_held() {
    let live = served();
    let mut socket = opened(&live).await;
    subscribe(&mut socket, "a", "open", "board").await;
    subscribe(&mut socket, "b", "open", "outline").await;
    assert_eq!(live.broadcaster.listener_count(), 2);

    socket.close().await;
    settled().await;

    assert_eq!(
        live.broadcaster.listener_count(),
        0,
        "nobody is left hearing for it"
    );
}
