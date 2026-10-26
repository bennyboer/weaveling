mod tombstones;

use std::sync::Arc;

use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use outline_catalog::InMemoryOutlineCatalog;
use outline_contract::EVERY_OUTLINE;
use outline_core::{OutlineCatalog, OutlineEvent, OutlinesCore};
use outline_messaging::{
    CatalogueOutlinesListener, DetachOnDiscardListener, DetachOnSceneDeletedListener,
    DiscardOutlinesOnProjectDeletedListener, IndexAttachmentsListener,
};
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, EventSourced, Feature, Wired};

use crate::tombstones::OutlineTombstones;

pub const LIVE: &str = "broadcast-outline-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub catalog: Arc<dyn OutlineCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> OutlinesCore {
    OutlinesCore::new(
        ports.events.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let outlines = core(ports, context);
    let catalogue = CatalogueOutlinesListener::new(outlines.clone());
    let index = IndexAttachmentsListener::new(outlines.clone());
    let detach_discarded_ideas = DetachOnDiscardListener::new(outlines.clone());
    let detach_deleted_scenes = DetachOnSceneDeletedListener::new(outlines.clone());
    let sweep = DiscardOutlinesOnProjectDeletedListener::new(outlines.clone());
    let broadcaster = Broadcaster::new();
    let broadcast = BroadcastChangesListener::new(LIVE, EVERY_OUTLINE, broadcaster.clone());

    Wired::serving(
        outline_rest::router(outlines.clone())
            .merge(outline_rest::live(outlines.clone(), broadcaster)),
    )
    .listening(vec![
        Arc::new(broadcast),
        Arc::new(catalogue),
        Arc::new(index),
        Arc::new(detach_discarded_ideas),
        Arc::new(detach_deleted_scenes),
        Arc::new(sweep),
    ])
    .doing_chores(vec![Arc::new(ForgetOldTombstones::new(
        "forget-old-outline-tombstones",
        Arc::new(OutlineTombstones::of(outlines)),
        context.clock.clone(),
    ))])
}

pub struct OutlineFeature;

impl Feature for OutlineFeature {
    const NAME: &'static str = "outline";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(outline_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryOutlineCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            outline_store::codec(),
            outline_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(outline_catalog::PostgresOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            outline_catalog::postgres::migrations(),
            registry::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            outline_store::codec(),
            outline_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(outline_catalog::SqliteOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::SqliteRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            outline_catalog::sqlite::migrations(),
            registry::sqlite::migrations(),
        ]
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
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use chores::{TOMBSTONES_KEPT_FOR, Tombstones};
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use outline_core::Attachment;
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    fn a_context() -> Context {
        Context {
            clock: Arc::new(FixedClock::new(
                NOW + TOMBSTONES_KEPT_FOR + Duration::SECOND,
            )),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
            authorizer: Arc::new(AllowingEverything),
        }
    }

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_as_a_chore() {
        let context = a_context();
        let ports = OutlineFeature::in_memory(&context);
        let wired = OutlineFeature::wire(&ports, &context);
        for (gone, laid) in [
            (Attachment::scene("scene_long_gone"), NOW),
            (Attachment::idea("idea_just_gone"), NOW + Duration::SECOND),
        ] {
            ports
                .catalog
                .lay_tombstone(&gone, laid)
                .await
                .expect("laying a tombstone should succeed");
        }

        let [chore] = wired.chores.as_slice() else {
            panic!("the feature hands over one chore");
        };

        assert_eq!(chore.named(), "forget-old-outline-tombstones");
        assert_eq!(
            chore
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }

    #[tokio::test]
    async fn the_tombstones_handed_to_the_chore_forget_no_more_than_a_batch() {
        let context = a_context();
        let ports = OutlineFeature::in_memory(&context);
        for gone in [Attachment::scene("scene_1"), Attachment::idea("idea_1")] {
            ports
                .catalog
                .lay_tombstone(&gone, NOW)
                .await
                .expect("laying a tombstone should succeed");
        }

        let forgotten = OutlineTombstones::of(core(&ports, &context))
            .forget_laid_before(NOW + Duration::SECOND, 1)
            .await
            .expect("forgetting tombstones should succeed");

        assert_eq!(forgotten, 1);
    }
}
