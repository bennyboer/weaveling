use axum::extract::ws::{Message as Frame, WebSocket};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::task::JoinHandle;

use crate::live_passages::{LivePassage, Overheard, PeerId, Reaction};

pub struct Peer {
    id: PeerId,
    to_peer: UnboundedSender<Vec<u8>>,
    writing: JoinHandle<()>,
    overhearing: JoinHandle<()>,
}

impl Peer {
    pub fn arrive(
        id: PeerId,
        mut sink: SplitSink<WebSocket, Frame>,
        passage: &LivePassage,
    ) -> Self {
        let (to_peer, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
        let mut traffic = passage.listen();

        let writing = tokio::spawn(async move {
            while let Some(frame) = outgoing.recv().await {
                if sink.send(Frame::Binary(frame.into())).await.is_err() {
                    break;
                }
            }
        });

        let overhearing = tokio::spawn({
            let to_peer = to_peer.clone();
            async move {
                while let Ok(overheard) = traffic.recv().await {
                    if overheard.from != id && to_peer.send(overheard.frame).is_err() {
                        break;
                    }
                }
            }
        });

        Peer {
            id,
            to_peer,
            writing,
            overhearing,
        }
    }

    pub fn id(&self) -> PeerId {
        self.id
    }

    pub fn deliver(&self, reaction: &Reaction, passage: &LivePassage) {
        if let Some(reply) = &reaction.to_sender {
            let _ = self.to_peer.send(reply.encode());
        }
        if let Some(shared) = &reaction.to_others {
            passage.broadcast(Overheard {
                from: self.id,
                frame: shared.encode(),
            });
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.overhearing.abort();
        self.writing.abort();
    }
}
