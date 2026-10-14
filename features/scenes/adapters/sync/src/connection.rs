use std::collections::HashMap;
use std::sync::Arc;

use scenes_core::SceneId;
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;

use crate::envelope::Envelope;
use crate::live_scenes::{LiveScenes, Presence};
use crate::peer::Peer;

pub struct Connection {
    live: LiveScenes,
    to_client: UnboundedSender<Vec<u8>>,
    joined: HashMap<String, (Peer, Presence)>,
}

impl Connection {
    pub fn new(live: LiveScenes, to_client: UnboundedSender<Vec<u8>>) -> Self {
        Self {
            live,
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
        let Some(present) = present else {
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
        self.joined.insert(scene, (peer, present));
    }

    async fn carry(&mut self, scene: &str, frame: &[u8]) {
        let Some((peer, present)) = self.joined.get(scene) else {
            warn!(%scene, "ignoring a frame for a scene this connection has not joined");
            return;
        };

        match present.react_to(frame) {
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
