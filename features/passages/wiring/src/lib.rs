use std::sync::Arc;

use passages_core::{PassageService, PassageStore};
use passages_store::InMemoryPassageStore;
use passages_sync::LivePassages;
use wiring::{Context, Wired};

pub struct Ports {
    pub store: Arc<dyn PassageStore>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemoryPassageStore::new()),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let passages = PassageService::new(ports.store.clone(), context.clock.clone());

    Wired::serving(
        passages_rest::router(passages.clone())
            .merge(passages_sync::router(LivePassages::new(passages))),
    )
}
