use std::sync::Arc;

use belonging::{Belongings, InMemoryBelongings};
use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use outline_catalog::InMemoryOutlineCatalog;
use outline_contract::EVERY_OUTLINE;
use outline_core::{OutlineCatalog, OutlineEvent, OutlinesCore};
use outline_messaging::{
    CatalogueOutlinesListener, DetachEverywhereListener, DetachGoneAttachmentsListener,
    DetachOnDiscardListener, DetachOnSceneDeletedListener, DiscardOutlinesOnProjectDeletedListener,
    IndexAttachmentsListener, RememberDeletedProjectsListener,
    RememberProjectsOfAttachmentsListener,
};
use registry::{InMemoryRegistry, Registry};
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Wired};

pub const LIVE: &str = "broadcast-outline-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<OutlineEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn OutlineCatalog>,
    pub registry: Arc<dyn Registry>,
    pub belongings: Arc<dyn Belongings>,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> OutlinesCore {
    OutlinesCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        ports.belongings.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let outlines = core(ports, context);
    let catalogue = hearing.inboxed(CatalogueOutlinesListener::new(outlines.clone()));
    let index = hearing.inboxed(IndexAttachmentsListener::new(
        outlines.clone(),
        ports.outbox.clone(),
        context.clock.clone(),
    ));
    let detach_discarded_ideas = hearing.inboxed(DetachOnDiscardListener::new(
        outlines.clone(),
        ports.outbox.clone(),
        context.clock.clone(),
    ));
    let detach_deleted_scenes = hearing.inboxed(DetachOnSceneDeletedListener::new(
        outlines.clone(),
        ports.outbox.clone(),
        context.clock.clone(),
    ));
    let detach_everywhere = hearing.inboxed(DetachEverywhereListener::new(outlines.clone()));
    let detach_gone = hearing.inboxed(DetachGoneAttachmentsListener::new(outlines.clone()));
    let remember_deleted_projects = hearing.inboxed(RememberDeletedProjectsListener::new(
        outlines.clone(),
        ports.outbox.clone(),
        context.clock.clone(),
    ));
    let remember_projects_of_attachments =
        hearing.inboxed(RememberProjectsOfAttachmentsListener::new(outlines.clone()));
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
            Arc::new(remember_projects_of_attachments),
            Arc::new(sweep),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-outline-tombstones",
                chores::tombstones_by(move |before, at_most| {
                    let outlines = outlines.clone();
                    async move {
                        outlines
                            .forget_tombstones_laid_before(before, at_most)
                            .await
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-outline-hearings", context.clock.clone())),
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
            belongings: Arc::new(InMemoryBelongings::new()),
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
            registry: Arc::new(registry::PostgresRegistry::new(pool.clone())),
            belongings: Arc::new(belonging::PostgresBelongings::new(pool)),
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
            belonging::postgres::migrations(),
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
            registry: Arc::new(registry::SqliteRegistry::new(pool.clone())),
            belongings: Arc::new(belonging::SqliteBelongings::new(pool)),
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
            belonging::sqlite::migrations(),
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
    use chores::TOMBSTONES_KEPT_FOR;
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
        // given: a feature whose clock is past how long tombstones are kept
        let context = a_context();
        let ports = OutlineFeature::in_memory(&context);
        let wired = OutlineFeature::wire(&ports, &context);
        // and: one tombstone laid long ago and one just now
        for (gone, laid) in [
            (Attachment::scene("scene_long_gone"), NOW),
            (Attachment::idea("idea_just_gone"), NOW + Duration::SECOND),
        ] {
            core(&ports, &context)
                .lay_tombstone(None, &gone, laid)
                .await
                .expect("laying a tombstone should succeed");
        }

        // when: its chores are handed over
        let [chore, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        // then: one forgets old hearings
        assert_eq!(hearings.named(), "forget-old-outline-hearings");

        // and: the other forgets only the long gone tombstone
        assert_eq!(chore.named(), "forget-old-outline-tombstones");
        assert_eq!(
            chore
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            2,
            "the long gone one's, and what it belonged to"
        );
    }
}
