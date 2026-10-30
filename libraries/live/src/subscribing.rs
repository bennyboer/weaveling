use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::{Extension, Router, routing};
use eventsourcing::Agent;
use live_contract::{ACCESS_CHANGED, AskedDTO, FrameDTO, ToldDTO, ended};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::following::Asked;
use crate::socket::{Access, Admission, Sieve, rechecked};
use crate::{Channel, Heard, Hearing};

const WAITING_TO_BE_SENT: usize = 64;

pub struct Subscribed {
    hearing: Box<dyn Heard>,
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

    pub fn standing_at(mut self, version: u64) -> Self {
        self.standing = Some(version);
        self
    }

    pub fn while_admitted(mut self, admission: Admission) -> Self {
        self.admission = Some(admission);
        self
    }

    pub fn sifted(mut self, sieve: Arc<dyn Sieve>) -> Self {
        self.sieve = Some(sieve);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NotThere,
    Failed,
}

#[async_trait]
pub trait Topic: Send + Sync {
    fn named(&self) -> &'static str;

    async fn subscribe(&self, agent: &Agent, about: &Asked) -> Result<Subscribed, Refused>;
}

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

pub fn live_socket(topics: Topics) -> Router {
    Router::new().route(
        "/live",
        routing::get(
            move |upgrade: WebSocketUpgrade, Extension(agent): Extension<Agent>| async move {
                let topics = topics.clone();
                let answer: Response =
                    upgrade.on_upgrade(move |socket| serve(socket, agent, topics));
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

async fn serve(mut socket: WebSocket, agent: Agent, topics: Topics) {
    let (sending, mut to_send) = mpsc::channel::<String>(WAITING_TO_BE_SENT);
    let mut following: HashMap<String, JoinHandle<()>> = HashMap::new();

    loop {
        tokio::select! {
            Some(said) = to_send.recv() => {
                if socket.send(Message::Text(said.into())).await.is_err() {
                    break;
                }
            }
            heard = socket.recv() => match heard {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<AskedDTO>(&text) {
                    Ok(AskedDTO::Subscribe { id, topic, about }) => {
                        if let Some(earlier) = following.remove(&id) {
                            earlier.abort();
                        }
                        let Some(topic) = topics.named(&topic) else {
                            let unknown = ended_as(&id, ended::UNKNOWN_TOPIC);
                            if socket.send(Message::Text(unknown.into())).await.is_err() {
                                break;
                            }
                            continue;
                        };
                        let about: Asked = about.into_iter().collect();
                        following.insert(
                            id.clone(),
                            tokio::spawn(subscription(id, topic, agent.clone(), about, sending.clone())),
                        );
                    }
                    Ok(AskedDTO::Unsubscribe { id }) => {
                        if let Some(ended) = following.remove(&id) {
                            ended.abort();
                        }
                    }
                    Err(_) => {}
                },
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }

    for ended in following.into_values() {
        ended.abort();
    }
}

async fn subscription(
    id: String,
    topic: Arc<dyn Topic>,
    agent: Agent,
    about: Asked,
    sending: mpsc::Sender<String>,
) {
    let subscribed = match topic.subscribe(&agent, &about).await {
        Ok(subscribed) => subscribed,
        Err(Refused::NotThere) => {
            _ = sending.send(ended_as(&id, ended::NOT_THERE)).await;
            return;
        }
        Err(Refused::Failed) => {
            _ = sending.send(ended_as(&id, ended::FAILED)).await;
            return;
        }
    };

    let why = follow(&id, subscribed, &sending).await;
    _ = sending.send(ended_as(&id, why)).await;
}

async fn follow(id: &str, subscribed: Subscribed, sending: &mpsc::Sender<String>) -> &'static str {
    let Subscribed {
        mut hearing,
        standing,
        mut admission,
        sieve,
    } = subscribed;
    let frame = |frame: Value| {
        told(&ToldDTO::Frame {
            id: id.to_owned(),
            frame,
        })
    };

    if let Some(version) = standing
        && sending
            .send(frame(json!({ "version": version })))
            .await
            .is_err()
    {
        return ended::FAILED;
    }

    loop {
        tokio::select! {
            access = rechecked(&mut admission) => match access {
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
                let Ok(read) = serde_json::from_str::<Value>(&heard) else {
                    return ended::FAILED;
                };
                if let Some(sieve) = &sieve {
                    let Ok(as_frame) = serde_json::from_value::<FrameDTO>(read.clone()) else {
                        return ended::FAILED;
                    };
                    match sieve.lets_through(&as_frame).await {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(_) => return ended::FAILED,
                    }
                }
                if sending.send(frame(read)).await.is_err() {
                    return ended::FAILED;
                }
            }
        }
    }
}
