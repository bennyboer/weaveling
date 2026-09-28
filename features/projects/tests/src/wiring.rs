use std::sync::Arc;

use axum::Router;
use clock::Clock;
use eventsourcing::{InMemoryEventStore, PublishingEventStore};
use messaging::{InProcessDispatcher, Listener};
use projects_catalog::InMemoryProjectCatalog;
use projects_core::{ProjectEvent, ProjectService};
use wiring::Context;

const CATALOGUING: &str = "catalogue-project";

pub struct Wired {
    pub projects: ProjectService,
    pub routes: Router,
    pub catalog: Arc<InMemoryProjectCatalog>,
    pub projector: Arc<dyn Listener>,
}

pub fn wired(clock: Arc<dyn Clock>) -> Wired {
    let store = Arc::new(InMemoryEventStore::<ProjectEvent>::new());
    let catalog = Arc::new(InMemoryProjectCatalog::new());
    let dispatcher = Arc::new(InProcessDispatcher::new());
    let ports = projects_wiring::Ports {
        events: PublishingEventStore::wrapping(
            store.clone(),
            Arc::new(projects_messaging::ProjectEventPublisher::new(
                dispatcher.clone(),
            )),
        ),
        catalog: catalog.clone(),
    };
    let context = Context {
        clock,
        publisher: dispatcher.clone(),
    };
    let wired = projects_wiring::wire(&ports, &context);

    for listener in &wired.listeners {
        dispatcher.listen(listener.clone());
    }

    let projector = wired
        .listeners
        .iter()
        .find(|listener| listener.named().as_str() == CATALOGUING)
        .cloned()
        .expect("the feature should wire a catalog projector");

    Wired {
        projects: projects_wiring::service(&ports, &context),
        routes: wired.routes,
        catalog,
        projector,
    }
}
