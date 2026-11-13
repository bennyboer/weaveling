use std::collections::HashMap;
use std::error::Error;
use std::future;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::{Extension, Router, routing};
use ending::Ends;
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, AskedDTO, FrameDTO, ToldDTO, ended};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::gate::{Access, Admission};
use crate::{Channel, Frames, Hearing};

const WAITING_TO_BE_SENT: usize = 64;
/// The largest message a client may send.
/// Clients only subscribe and unsubscribe, so their messages are far smaller than this.
/// The limit keeps anyone from filling the server's memory with one message.
pub const LARGEST_ASKED: usize = 64 * 1024;

/// What a subscription asks to follow, as named values.
/// For example `{ "project": … }`, `{ "board": … }` or `{ "mine": "true" }`.
pub type About = HashMap<String, String>;

/// Lets each frame through to one follower, or holds it back.
/// This way followers of one channel can each receive only their own part of it.
/// An error ends the subscription as failed, rather than skipping a frame the follower may have needed.
#[async_trait]
pub trait Sieve: Send + Sync {
    async fn lets_through(&self, frame: &FrameDTO) -> Result<bool, Box<dyn Error + Send + Sync>>;
}

/// What a topic returns when it lets a subscription in.
/// It holds the frames the subscription receives, and how they are to be followed.
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

    /// First tells the follower the version it is at.
    /// This way a change made between reading and subscribing is not lost.
    pub fn standing_at(mut self, version: u64) -> Self {
        self.standing = Some(version);
        self
    }

    /// Follows only while the admission holds.
    /// The admission is checked again whenever access may have changed.
    /// If access is gone, the subscription ends with `access-lost`.
    /// If access remains, the follower is sent `access-changed`.
    pub fn while_admitted(mut self, admission: Admission) -> Self {
        self.admission = Some(admission);
        self
    }

    /// Passes each frame through `sieve` before sending it.
    pub fn sifted(mut self, sieve: Arc<dyn Sieve>) -> Self {
        self.sieve = Some(sieve);
        self
    }
}

/// Why a topic turned a subscription away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Nothing to follow, or nothing this agent may follow.
    /// Both cases look the same, so a stranger learns nothing.
    NotThere,
    /// It could not be decided whether it may be followed.
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

/// One feature's part of the shared socket: its name there, and who may follow what.
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

/// Serves `/live`, the one socket a client uses to subscribe to every topic.
/// It closes when its request's `Ends` say so.
pub fn live_socket(topics: Topics) -> Router {
    Router::new().route(
        "/live",
        routing::get(
            move |upgrade: WebSocketUpgrade,
                  Extension(agent): Extension<Agent>,
                  ends: Ends| async move {
                let topics = topics.clone();
                let answer: Response = upgrade
                    .max_message_size(LARGEST_ASKED)
                    .max_frame_size(LARGEST_ASKED)
                    .on_upgrade(move |socket| serve(socket, agent, topics, ends));
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

async fn serve(mut socket: WebSocket, agent: Agent, topics: Topics, ends: Ends) {
    let (sending, mut to_send) = mpsc::channel::<String>(WAITING_TO_BE_SENT);
    let mut following = Following::new();
    let ended = ends.reached();
    tokio::pin!(ended);

    loop {
        tokio::select! {
            end = &mut ended => {
                _ = socket.send(Message::Close(Some(end.frame()))).await;
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

async fn access_changed(admission: &mut Option<Admission>) {
    match admission {
        Some(admission) => admission.changed().await,
        None => future::pending().await,
    }
}

async fn asked_again(admission: &Option<Admission>) -> Access {
    match admission {
        Some(admission) => admission.asked_again().await,
        None => Access::Kept,
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

    let following = told(&ToldDTO::Following { id: id.to_owned() });
    if sending.send(following).await.is_err() {
        return ended::FAILED;
    }
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
            () = access_changed(&mut admission) => match asked_again(&admission).await {
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
