use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use passages_core::{Passage, PassageId, PassageService, PassageServiceError};
use tokio::sync::broadcast;

use crate::room::Room;

pub type PeerId = usize;

type Hydrated = HashMap<PassageId, Arc<LivePassage>>;

const BACKLOG: usize = 256;

#[derive(Debug, Clone)]
pub struct Overheard {
    pub from: PeerId,
    pub frame: Vec<u8>,
}

pub struct LivePassage {
    pub room: Room,
    pub traffic: broadcast::Sender<Overheard>,
}

#[derive(Clone)]
pub struct LivePassages {
    hydrated: Arc<RwLock<Hydrated>>,
    service: PassageService,
}

impl LivePassages {
    pub fn new(service: PassageService) -> Self {
        LivePassages {
            hydrated: Arc::new(RwLock::new(Hydrated::new())),
            service,
        }
    }

    pub async fn hydrate(&self, id: PassageId) -> Result<Arc<LivePassage>, PassageServiceError> {
        if let Some(found) = self.look(id) {
            return Ok(found);
        }

        let passage = self.service.open(&id.to_string()).await?;

        Ok(self.settle(id, passage))
    }

    pub async fn persist(&self, id: PassageId, update: &[u8]) -> Result<(), PassageServiceError> {
        self.service.absorb(&id.to_string(), update).await
    }

    fn look(&self, id: PassageId) -> Option<Arc<LivePassage>> {
        self.hydrated
            .read()
            .expect("live passages lock poisoned")
            .get(&id)
            .cloned()
    }

    fn settle(&self, id: PassageId, passage: Passage) -> Arc<LivePassage> {
        self.hydrated
            .write()
            .expect("live passages lock poisoned")
            .entry(id)
            .or_insert_with(|| {
                Arc::new(LivePassage {
                    room: Room::adopt(passage),
                    traffic: broadcast::channel(BACKLOG).0,
                })
            })
            .clone()
    }
}
