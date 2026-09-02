use std::sync::Arc;

use axum::Router;
use clock::Clock;
use passages_core::{PassageService, PassageStore};
use passages_store::InMemoryPassageStore;
use passages_sync::LivePassages;

pub struct Ports {
    pub store: Arc<dyn PassageStore>,
}

pub struct Wired {
    pub routes: Router,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemoryPassageStore::new()),
        }
    }
}

pub fn wire(ports: Ports, clock: Arc<dyn Clock>) -> Wired {
    let passages = PassageService::new(ports.store, clock);

    Wired {
        routes: passages_rest::router(passages.clone())
            .merge(passages_sync::router(LivePassages::new(passages))),
    }
}
