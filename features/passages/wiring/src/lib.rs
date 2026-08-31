use std::sync::Arc;

use axum::Router;
use clock::Clock;
use passages_core::{PassageService, PassageStore};
use passages_sync::LivePassages;

pub struct Wired {
    pub routes: Router,
}

pub fn wire(store: Arc<dyn PassageStore>, clock: Arc<dyn Clock>) -> Wired {
    let passages = PassageService::new(store, clock);

    Wired {
        routes: passages_rest::router(passages.clone())
            .merge(passages_sync::router(LivePassages::new(passages))),
    }
}
