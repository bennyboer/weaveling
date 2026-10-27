use std::collections::HashMap;
use std::future;
use std::sync::Arc;

use authorizing::AccessWatch;
use eventsourcing::Agent;
use futures_util::future::select_all;
use scenes_core::{Access, SceneId};
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;

use crate::envelope::Envelope;
use crate::live_scenes::{LiveScenes, Presence};
use crate::peer::Peer;
use crate::protocol::Message;

pub struct Connection {
    live: LiveScenes,
    agent: Agent,
    to_client: UnboundedSender<Vec<u8>>,
    joined: HashMap<String, Joined>,
}

struct Joined {
    peer: Peer,
    present: Presence,
    access: Access,
    watch: AccessWatch,
}

impl Connection {
    pub fn new(live: LiveScenes, agent: Agent, to_client: UnboundedSender<Vec<u8>>) -> Self {
        Self {
            live,
            agent,
            to_client,
            joined: HashMap::new(),
        }
    }

    pub async fn hear(&mut self, bytes: &[u8]) {
        match Envelope::decode(bytes) {
            Ok(Envelope::Join(scene)) => self.join(scene).await,
            Ok(Envelope::Leave(scene)) => {
                self.joined.remove(&scene);
            }
            Ok(Envelope::Carry { scene, frame }) => self.carry(&scene, &frame).await,
            Ok(Envelope::Refused(scene)) => {
                warn!(%scene, "a client sent what only the server sends");
            }
            Err(problem) => warn!(%problem, "ignoring an envelope"),
        }
    }

    async fn join(&mut self, scene: String) {
        if self.joined.contains_key(&scene) {
            return;
        }

        let present = match scene.parse::<SceneId>() {
            Ok(id) => self.live.join(id).await.ok(),
            Err(_) => None,
        };
        let admitted = match present {
            Some(present) => match (present.watch_access(), present.access(&self.agent).await) {
                (_, Ok(Access::Stranger)) => None,
                (watch, Ok(access)) => Some((present, access, watch)),
                (_, Err(problem)) => {
                    warn!(%scene, %problem, "who may join a scene could not be asked");
                    None
                }
            },
            None => None,
        };
        let Some((present, access, watch)) = admitted else {
            let _ = self.to_client.send(Envelope::Refused(scene).encode());
            return;
        };

        let wrapping = {
            let scene = scene.clone();

            Arc::new(move |frame| {
                Envelope::Carry {
                    scene: scene.clone(),
                    frame,
                }
                .encode()
            })
        };
        let peer = Peer::arrive(
            self.live.next_peer(),
            self.to_client.clone(),
            wrapping,
            &present,
        );
        peer.deliver(&present.greet(), &present);
        self.joined.insert(
            scene,
            Joined {
                peer,
                present,
                access,
                watch,
            },
        );
    }

    pub async fn access_changed(&mut self) -> String {
        if self.joined.is_empty() {
            return future::pending().await;
        }
        let watching = self.joined.iter_mut().map(|(scene, joined)| {
            Box::pin(async move {
                joined.watch.changed().await;
                scene.clone()
            })
        });

        select_all(watching).await.0
    }

    pub async fn ask_again(&mut self, scene: String) {
        let Some(joined) = self.joined.get_mut(&scene) else {
            return;
        };

        match joined.present.access(&self.agent).await {
            Ok(Access::Stranger) => {}
            Ok(access) => {
                joined.access = access;
                return;
            }
            Err(problem) => warn!(%scene, %problem, "who may stay in a scene could not be asked"),
        }
        self.joined.remove(&scene);
        let _ = self.to_client.send(Envelope::Refused(scene).encode());
    }

    async fn carry(&mut self, scene: &str, frame: &[u8]) {
        let Some(Joined {
            peer,
            present,
            access,
            ..
        }) = self.joined.get(scene)
        else {
            warn!(%scene, "ignoring a frame for a scene this connection has not joined");
            return;
        };
        let message = match Message::decode(frame) {
            Ok(message) => message,
            Err(problem) => {
                warn!(peer = peer.id(), %problem, "ignoring a frame");
                return;
            }
        };
        if *access == Access::Reader && message.changes_the_text() {
            return;
        }

        match present.receive(message) {
            Ok(reaction) => {
                peer.deliver(&reaction, present);

                if let Some(update) = reaction.to_store
                    && let Err(problem) = present.persist(&update).await
                {
                    warn!(peer = peer.id(), %problem, "an edit reached the scene but not the store");
                }
            }
            Err(problem) => warn!(peer = peer.id(), %problem, "ignoring a frame"),
        }
    }
}
