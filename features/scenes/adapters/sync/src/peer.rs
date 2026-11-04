use std::sync::Arc;

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::live_scenes::{LiveScene, Overheard, PeerId, Reaction};
use crate::protocol::Message;

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
        scene: &Arc<LiveScene>,
    ) -> Self {
        let mut traffic = scene.listen();

        let overhearing = tokio::spawn({
            let to_peer = to_peer.clone();
            let wrapping = wrapping.clone();
            let scene = scene.clone();
            async move {
                loop {
                    let frame = match traffic.recv().await {
                        Ok(overheard) if overheard.from == id => continue,
                        Ok(overheard) => overheard.frame,
                        Err(RecvError::Lagged(_)) => {
                            Message::JustHappened(scene.everything()).encode()
                        }
                        Err(RecvError::Closed) => break,
                    };
                    if to_peer.send(wrapping(frame)).is_err() {
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use scenes_core::{ProjectLink, Scene, SceneId};
    use time::OffsetDateTime;
    use tokio::sync::mpsc;

    use super::*;
    use crate::live_scenes::BACKLOG;

    #[tokio::test]
    async fn a_peer_that_fell_behind_is_sent_the_whole_scene_and_hears_on() {
        let scene = Arc::new(LiveScene::open(Scene::empty(
            SceneId::generate(OffsetDateTime::UNIX_EPOCH),
            ProjectLink::from("project_1"),
        )));
        let (to_peer, mut heard) = mpsc::unbounded_channel();
        let _peer = Peer::arrive(1, to_peer, Arc::new(|frame| frame), &scene);

        for _ in 0..BACKLOG + 10 {
            scene.broadcast(Overheard {
                from: 2,
                frame: vec![1],
            });
        }
        let caught_up = tokio::time::timeout(Duration::from_secs(1), heard.recv())
            .await
            .expect("a peer behind is told at once")
            .expect("the peer is still heard");
        scene.broadcast(Overheard {
            from: 2,
            frame: vec![9],
        });

        assert!(
            matches!(Message::decode(&caught_up), Ok(Message::JustHappened(_))),
            "what it missed is sent whole, as an edit, since the room cannot know what the peer holds"
        );
        let mut later = Vec::new();
        while later != vec![9] {
            later = tokio::time::timeout(Duration::from_secs(1), heard.recv())
                .await
                .expect("a peer that fell behind hears on")
                .expect("the peer is still heard");
        }
    }
}
