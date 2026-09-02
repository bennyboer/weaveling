use std::sync::Arc;

use axum::Router;
use clock::Clock;
use projects_core::{ProjectService, ProjectStore};
use projects_store::InMemoryProjectStore;

pub struct Ports {
    pub store: Arc<dyn ProjectStore>,
}

pub struct Wired {
    pub routes: Router,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemoryProjectStore::new()),
        }
    }
}

pub fn wire(ports: Ports, clock: Arc<dyn Clock>) -> Wired {
    Wired {
        routes: projects_rest::router(ProjectService::new(ports.store, clock)),
    }
}
