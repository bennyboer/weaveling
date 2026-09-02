use std::sync::Arc;

use projects_core::{ProjectService, ProjectStore};
use projects_store::InMemoryProjectStore;
use wiring::{Context, Wired};

pub struct Ports {
    pub store: Arc<dyn ProjectStore>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(InMemoryProjectStore::new()),
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let projects = ProjectService::new(ports.store.clone(), context.clock.clone());

    Wired::serving(projects_rest::router(projects))
}
