use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::live_scenes::{LiveScene, Overheard, PeerId, Reaction};

pub type Wrapping = Arc<dyn Fn(Vec<u8>) -> Vec<u8> + Send + Sync>;

pub struct Peer {
    id: PeerId,
    to_peer: UnboundedSender<Vec<u8>>,
    wrapping: Wrapping,
    overhearing: JoinHandle<()>,
}

impl Peer {
    pub fn arrive(
        id: PeerId,
        to_peer: UnboundedSender<Vec<u8>>,
        wrapping: Wrapping,
        scene: &LiveScene,
    ) -> Self {
        let mut traffic = scene.listen();

        let overhearing = tokio::spawn({
            let to_peer = to_peer.clone();
            let wrapping = wrapping.clone();
            async move {
                while let Ok(overheard) = traffic.recv().await {
                    if overheard.from != id && to_peer.send(wrapping(overheard.frame)).is_err() {
                        break;
                    }
                }
            }
        });

        Peer {
            id,
            to_peer,
            wrapping,
            overhearing,
        }
    }

    pub fn id(&self) -> PeerId {
        self.id
    }

    pub fn deliver(&self, reaction: &Reaction, scene: &LiveScene) {
        if let Some(reply) = &reaction.to_sender {
            let _ = self.to_peer.send((self.wrapping)(reply.encode()));
        }
        if let Some(shared) = &reaction.to_others {
            scene.broadcast(Overheard {
                from: self.id,
                frame: shared.encode(),
            });
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.overhearing.abort();
    }
}
