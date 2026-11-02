mod tombstones;

use std::sync::Arc;

use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use inbox::{HeardMessages, InMemoryInbox, Inbox, Inboxed};
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use outline_catalog::InMemoryOutlineCatalog;
use outline_contract::EVERY_OUTLINE;
use outline_core::{OutlineCatalog, OutlineEvent, OutlinesCore};
use outline_messaging::{
    CatalogueOutlinesListener, DetachEverywhereListener, DetachGoneAttachmentsListener,
    DetachOnDiscardListener, DetachOnSceneDeletedListener, DiscardOutlinesOnProjectDeletedListener,
    IndexAttachmentsListener, RememberDeletedProjectsListener,
};
use registry::{InMemoryRegistry, Registry};
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Wired};

use crate::tombstones::OutlineTombstones;

pub const LIVE: &str = "broadcast-outline-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn OutlineCatalog>,
    pub registry: Arc<dyn Registry>,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> OutlinesCore {
    OutlinesCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let outlines = core(ports, context);
    let catalogue = Inboxed::new(
        CatalogueOutlinesListener::new(outlines.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let index = Inboxed::new(
        IndexAttachmentsListener::new(
            outlines.clone(),
            ports.outbox.clone(),
            context.clock.clone(),
        ),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let detach_discarded_ideas = Inboxed::new(
        DetachOnDiscardListener::new(
            outlines.clone(),
            ports.outbox.clone(),
            context.clock.clone(),
        ),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let detach_deleted_scenes = Inboxed::new(
        DetachOnSceneDeletedListener::new(
            outlines.clone(),
            ports.outbox.clone(),
            context.clock.clone(),
        ),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let detach_everywhere = Inboxed::new(
        DetachEverywhereListener::new(outlines.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let detach_gone = Inboxed::new(
        DetachGoneAttachmentsListener::new(outlines.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let remember_deleted_projects = Inboxed::new(
        RememberDeletedProjectsListener::new(
            outlines.clone(),
            ports.outbox.clone(),
            context.clock.clone(),
        ),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let sweep = DiscardOutlinesOnProjectDeletedListener::new(outlines.clone());
    let broadcaster = Broadcaster::new();
    let broadcast =
        BroadcastChangesListener::new(LIVE, EVERY_OUTLINE, broadcaster.clone(), |published| {
            vec![outline_rest::OutlineChanges(published.aggregate.id.clone())]
        });

    Wired::serving(outline_rest::router(outlines.clone()))
        .followed_live(vec![Arc::new(outline_rest::OutlinesTopic::new(
            outlines.clone(),
            broadcaster,
        ))])
        .listening(vec![
            Arc::new(broadcast),
            Arc::new(catalogue),
            Arc::new(index),
            Arc::new(detach_discarded_ideas),
            Arc::new(detach_deleted_scenes),
            Arc::new(detach_everywhere),
            Arc::new(detach_gone),
            Arc::new(remember_deleted_projects),
            Arc::new(sweep),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-outline-tombstones",
                Arc::new(OutlineTombstones::of(outlines)),
                context.clock.clone(),
            )),
            Arc::new(ForgetOldTombstones::new(
                "forget-old-outline-hearings",
                Arc::new(HeardMessages::of(
                    ports.inbox.clone(),
                    ports.transactions.clone(),
                )),
                context.clock.clone(),
            )),
        ])
}

pub struct OutlineFeature;

impl Feature for OutlineFeature {
    const NAME: &'static str = "outline";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(outline_messaging::message_for, context);

        Ports {
            events,
            transactions,
            catalog: Arc::new(InMemoryOutlineCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_postgres(
            &pool,
            outline_store::codec(),
            outline_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(outline_catalog::PostgresOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool)),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
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
            inbox::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_sqlite(
            &pool,
            outline_store::codec(),
            outline_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(outline_catalog::SqliteOutlineCatalog::new(pool.clone())),
            registry: Arc::new(registry::SqliteRegistry::new(pool)),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
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
            inbox::sqlite::migrations(),
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
    async fn the_feature_forgets_its_old_tombstones_and_hearings_as_chores() {
        let context = a_context();
        let ports = OutlineFeature::in_memory(&context);
        let wired = OutlineFeature::wire(&ports, &context);
        for (gone, laid) in [
            (Attachment::scene("scene_long_gone"), NOW),
            (Attachment::idea("idea_just_gone"), NOW + Duration::SECOND),
        ] {
            core(&ports, &context)
                .lay_tombstone(None, &gone, laid)
                .await
                .expect("laying a tombstone should succeed");
        }

        let [chore, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        assert_eq!(hearings.named(), "forget-old-outline-hearings");

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
            core(&ports, &context)
                .lay_tombstone(None, &gone, NOW)
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
