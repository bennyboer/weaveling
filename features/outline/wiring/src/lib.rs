use std::sync::Arc;

use eventsourcing::{EventStore, InMemoryEventStore};
use outline_catalog::InMemoryOutlineCatalog;
use outline_core::{OutlineCatalog, OutlineEvent, OutlineService};
use outline_messaging::{
    AttachedPiecesProjector, DetachOnDiscard, OutlineCatalogProjector, Publishing,
};
use wiring::{Context, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub catalog: Arc<dyn OutlineCatalog>,
}

impl Ports {
    pub fn in_memory() -> Self {
        Self {
            events: Arc::new(InMemoryEventStore::new()),
            catalog: Arc::new(InMemoryOutlineCatalog::new()),
        }
    }
}

pub fn service(ports: &Ports, context: &Context) -> OutlineService {
    OutlineService::new(
        ports.events.clone(),
        ports.catalog.clone(),
        Arc::new(Publishing::new(context.publisher.clone())),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let outlines = service(ports, context);
    let catalogue = OutlineCatalogProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let index = AttachedPiecesProjector::new(
        ports.events.clone(),
        ports.catalog.clone(),
        context.clock.clone(),
    );
    let tidy = DetachOnDiscard::new(outlines.clone(), ports.catalog.clone());

    Wired::serving(outline_rest::router(outlines)).listening(vec![
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(tidy),
    ])
}
