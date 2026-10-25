use std::collections::HashMap;
use std::sync::Arc;

use eventsourcing::Agent;
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
    joined: HashMap<String, (Peer, Presence, Access)>,
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
            Some(present) => match present.access(&self.agent).await {
                Ok(Access::Stranger) => None,
                Ok(access) => Some((present, access)),
                Err(problem) => {
                    warn!(%scene, %problem, "who may join a scene could not be asked");
                    None
                }
            },
            None => None,
        };
        let Some((present, access)) = admitted else {
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
        self.joined.insert(scene, (peer, present, access));
    }

    async fn carry(&mut self, scene: &str, frame: &[u8]) {
        let Some((peer, present, access)) = self.joined.get(scene) else {
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
