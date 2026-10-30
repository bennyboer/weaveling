use std::collections::HashMap;
use std::error::Error;
use std::future;
use std::sync::Arc;

use acting::SessionEnd;
use async_trait::async_trait;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::{Extension, Router, routing};
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, AskedDTO, FrameDTO, SESSION_ENDED, ToldDTO, ended};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::gate::{Access, Admission};
use crate::{Channel, Frames, Hearing};

const WAITING_TO_BE_SENT: usize = 64;

/// What a subscription asks to follow, by name: `{ "project": … }`, `{ "board": … }`, `{ "mine": "true" }`.
pub type About = HashMap<String, String>;

/// Lets each frame through to one follower or holds it back, so that followers of one channel may each hear only their own part of it; an error ends the subscription as failed rather than skip a frame it may have needed.
#[async_trait]
pub trait Sieve: Send + Sync {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>>;
}

/// What a topic answers a subscription it lets in with: the frames it hears, and how they are to be followed.
pub struct Subscribed {
    hearing: Box<dyn Frames>,
    standing: Option<u64>,
    admission: Option<Admission>,
    sieve: Option<Arc<dyn Sieve>>,
}

impl Subscribed {
    pub fn hearing<C: Channel>(hearing: Hearing<C>) -> Self {
        Self {
            hearing: Box::new(hearing),
            standing: None,
            admission: None,
            sieve: None,
        }
    }

    /// Tells the follower first the version it stands at, so that a change made between reading and subscribing is not lost.
    pub fn standing_at(mut self, version: u64) -> Self {
        self.standing = Some(version);
        self
    }

    /// Follows only while the admission holds: asked again whenever access may have changed, ended `access-lost` once it no longer does, told `access-changed` when it still does.
    pub fn while_admitted(mut self, admission: Admission) -> Self {
        self.admission = Some(admission);
        self
    }

    /// Passes each frame through `sieve` before telling it.
    pub fn sifted(mut self, sieve: Arc<dyn Sieve>) -> Self {
        self.sieve = Some(sieve);
        self
    }
}

/// Why a topic turned a subscription away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Nothing to follow, or nothing this agent may follow: the two are told alike, so that a stranger learns nothing.
    NotThere,
    /// Whether it may be followed could not be told.
    Failed,
}

impl Refused {
    fn why(self) -> &'static str {
        match self {
            Self::NotThere => ended::NOT_THERE,
            Self::Failed => ended::FAILED,
        }
    }
}

/// One feature's part of the one socket: what it is called there, and whom it lets follow what.
#[async_trait]
pub trait Topic: Send + Sync {
    /// The name a client subscribes by.
    fn named(&self) -> &'static str;

    async fn subscribe(&self, agent: &Agent, about: &About) -> Result<Subscribed, Refused>;
}

/// The topics one socket serves, by their names.
#[derive(Clone, Default)]
pub struct Topics(Arc<HashMap<&'static str, Arc<dyn Topic>>>);

impl Topics {
    pub fn of(topics: Vec<Arc<dyn Topic>>) -> Self {
        Self(Arc::new(
            topics
                .into_iter()
                .map(|topic| (topic.named(), topic))
                .collect(),
        ))
    }

    fn named(&self, topic: &str) -> Option<Arc<dyn Topic>> {
        self.0.get(topic).cloned()
    }
}

/// Serves `/live`, the one socket a client subscribes to every topic over; it closes as `SESSION_ENDED` when the request's `SessionEnd` is reached.
pub fn live_socket(topics: Topics) -> Router {
    Router::new().route(
        "/live",
        routing::get(
            move |upgrade: WebSocketUpgrade,
                  Extension(agent): Extension<Agent>,
                  session: Option<Extension<SessionEnd>>| async move {
                let topics = topics.clone();
                let session = session.map_or_else(SessionEnd::never, |Extension(session)| session);
                let answer: Response =
                    upgrade.on_upgrade(move |socket| serve(socket, agent, topics, session));
                answer
            },
        ),
    )
}

fn told(said: &ToldDTO) -> String {
    serde_json::to_string(said).expect("what is told is plain data")
}

fn ended_as(id: &str, why: &str) -> String {
    told(&ToldDTO::Ended {
        id: id.to_owned(),
        why: why.to_owned(),
    })
}

type Following = HashMap<String, JoinHandle<()>>;

async fn serve(mut socket: WebSocket, agent: Agent, topics: Topics, session: SessionEnd) {
    let (sending, mut to_send) = mpsc::channel::<String>(WAITING_TO_BE_SENT);
    let mut following = Following::new();
    let session_ended = session.reached();
    tokio::pin!(session_ended);

    loop {
        tokio::select! {
            () = &mut session_ended => {
                _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: SESSION_ENDED,
                        reason: "session-ended".into(),
                    })))
                    .await;
                break;
            }
            Some(said) = to_send.recv() => {
                if socket.send(Message::Text(said.into())).await.is_err() {
                    break;
                }
            }
            heard = socket.recv() => match heard {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(asked) = serde_json::from_str::<AskedDTO>(&text) {
                        answer(asked, &mut following, &topics, &agent, &sending);
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }

    for ended in following.into_values() {
        ended.abort();
    }
}

fn answer(
    asked: AskedDTO,
    following: &mut Following,
    topics: &Topics,
    agent: &Agent,
    sending: &mpsc::Sender<String>,
) {
    let (AskedDTO::Subscribe { id, .. } | AskedDTO::Unsubscribe { id }) = &asked;
    if let Some(earlier) = following.remove(id) {
        earlier.abort();
    }

    if let AskedDTO::Subscribe { id, topic, about } = asked {
        following.insert(
            id.clone(),
            tokio::spawn(subscription(
                id,
                topics.named(&topic),
                agent.clone(),
                about.into_iter().collect(),
                sending.clone(),
            )),
        );
    }
}

async fn subscription(
    id: String,
    topic: Option<Arc<dyn Topic>>,
    agent: Agent,
    about: About,
    sending: mpsc::Sender<String>,
) {
    let why = match topic {
        None => ended::UNKNOWN_TOPIC,
        Some(topic) => match topic.subscribe(&agent, &about).await {
            Ok(subscribed) => follow(&id, subscribed, &sending).await,
            Err(refused) => refused.why(),
        },
    };

    _ = sending.send(ended_as(&id, why)).await;
}

async fn access_rechecked(admission: &mut Option<Admission>) -> Access {
    match admission {
        Some(admission) => admission.rechecked().await,
        None => future::pending().await,
    }
}

async fn follow(id: &str, subscribed: Subscribed, sending: &mpsc::Sender<String>) -> &'static str {
    let Subscribed {
        mut hearing,
        standing,
        mut admission,
        sieve,
    } = subscribed;
    let framed = |frame: FrameDTO| {
        told(&ToldDTO::Frame {
            id: id.to_owned(),
            frame,
        })
    };

    if let Some(version) = standing {
        let standing = FrameDTO {
            of: None,
            version,
            at: None,
            event: Value::Null,
        };
        if sending.send(framed(standing)).await.is_err() {
            return ended::FAILED;
        }
    }

    loop {
        tokio::select! {
            access = access_rechecked(&mut admission) => match access {
                Access::Kept => {
                    let notice = told(&ToldDTO::Notice {
                        id: id.to_owned(),
                        notice: ACCESS_CHANGED.to_owned(),
                    });
                    if sending.send(notice).await.is_err() {
                        return ended::FAILED;
                    }
                }
                Access::Lost => return ended::ACCESS_LOST,
            },
            heard = hearing.next() => {
                let Some(heard) = heard else {
                    return ended::BEHIND;
                };
                let Ok(frame) = serde_json::from_str::<FrameDTO>(&heard) else {
                    return ended::FAILED;
                };
                if let Some(sieve) = &sieve {
                    match sieve.lets_through(&frame).await {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(_) => return ended::FAILED,
                    }
                }
                if sending.send(framed(frame)).await.is_err() {
                    return ended::FAILED;
                }
            }
        }
    }
}
