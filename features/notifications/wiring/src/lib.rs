use std::sync::Arc;

use axum::Router;
use eventsourcing::EventStore;
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{NoticeCatalog, NoticeEvent, NotificationsCore};
use notifications_messaging::{CatalogueNoticeListener, NoteRoleChangedListener};
use outbox::Outbox;
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub events: Arc<dyn EventStore<NoticeEvent>>,
    pub catalog: Arc<dyn NoticeCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> NotificationsCore {
    NotificationsCore::new(
        ports.events.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let notifications = core(ports, context);

    Wired::serving(Router::new()).listening(vec![
        Arc::new(NoteRoleChangedListener::new(notifications.clone())),
        Arc::new(CatalogueNoticeListener::new(notifications)),
    ])
}

pub struct NotificationsFeature;

impl Feature for NotificationsFeature {
    const NAME: &'static str = "notifications";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(notifications_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryNoticeCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}

#[cfg(test)]
mod tests {
    use authorizing::AllowingEverything;
    use clock::SystemClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};

    use super::*;

    fn a_context() -> Context {
        Context {
            clock: Arc::new(SystemClock),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
            authorizer: Arc::new(AllowingEverything),
        }
    }

    #[test]
    fn the_feature_notes_a_changed_role_lists_every_notice_and_serves_nothing_yet() {
        let context = a_context();

        let wired =
            NotificationsFeature::wire(&NotificationsFeature::in_memory(&context), &context);

        let named: Vec<String> = wired
            .listeners
            .iter()
            .map(|listener| listener.named().as_str().to_owned())
            .collect();
        assert_eq!(named, ["note-role-changed", "catalogue-notice"]);
        assert!(wired.topics.is_empty());
        assert!(wired.chores.is_empty());
    }
}
