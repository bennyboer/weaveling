mod tombstones;

use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_contract::EVERY_BOARD;
use boards_core::{BoardCatalog, BoardEvent, BoardsCore};
use boards_messaging::{
    CatalogueBoardsListener, DiscardBoardsOnProjectDeletedListener, ProjectPinnedIdeasListener,
    UnpinOnDiscardListener,
};
use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use registry::{InMemoryRegistry, Registry};
use wiring::{Context, EventSourced, Feature, Wired};

use crate::tombstones::BoardTombstones;

pub const LIVE: &str = "broadcast-board-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub catalog: Arc<dyn BoardCatalog>,
    pub registry: Arc<dyn Registry>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> BoardsCore {
    BoardsCore::new(
        ports.events.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let boards = core(ports, context);
    let catalogue = CatalogueBoardsListener::new(boards.clone());
    let index = ProjectPinnedIdeasListener::new(boards.clone());
    let unpin_discarded_ideas = UnpinOnDiscardListener::new(boards.clone());
    let sweep = DiscardBoardsOnProjectDeletedListener::new(boards.clone());
    let broadcaster = Broadcaster::new();
    let broadcast =
        BroadcastChangesListener::new(LIVE, EVERY_BOARD, broadcaster.clone(), |published| {
            vec![boards_rest::BoardChanges(published.aggregate.id.clone())]
        });

    Wired::serving(boards_rest::router(boards.clone()))
        .followed_live(vec![Arc::new(boards_rest::BoardsTopic::new(
            boards.clone(),
            broadcaster,
        ))])
        .listening(vec![
            Arc::new(broadcast),
            Arc::new(catalogue),
            Arc::new(index),
            Arc::new(unpin_discarded_ideas),
            Arc::new(sweep),
        ])
        .doing_chores(vec![Arc::new(ForgetOldTombstones::new(
            "forget-old-board-tombstones",
            Arc::new(BoardTombstones::of(boards)),
            context.clock.clone(),
        ))])
}

pub struct BoardFeature;

impl Feature for BoardFeature {
    const NAME: &'static str = "boards";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(boards_messaging::message_for, context);

        Ports {
            events,
            catalog: Arc::new(InMemoryBoardCatalog::new()),
            registry: Arc::new(InMemoryRegistry::new()),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_postgres(
            &pool,
            boards_store::codec(),
            boards_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(boards_catalog::PostgresBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::PostgresRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            boards_catalog::postgres::migrations(),
            registry::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced { events, outbox } = EventSourced::on_sqlite(
            &pool,
            boards_store::codec(),
            boards_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            catalog: Arc::new(boards_catalog::SqliteBoardCatalog::new(pool.clone())),
            registry: Arc::new(registry::SqliteRegistry::new(pool)),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            boards_catalog::sqlite::migrations(),
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
    use boards_core::IdeaLink;
    use chores::{TOMBSTONES_KEPT_FOR, Tombstones};
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
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
        let ports = BoardFeature::in_memory(&context);
        let wired = BoardFeature::wire(&ports, &context);
        for (idea, laid) in [
            ("idea_long_gone", NOW),
            ("idea_just_gone", NOW + Duration::SECOND),
        ] {
            ports
                .catalog
                .lay_tombstone(&IdeaLink::from(idea), laid)
                .await
                .expect("laying a tombstone should succeed");
        }

        let [chore] = wired.chores.as_slice() else {
            panic!("the feature hands over one chore");
        };

        assert_eq!(chore.named(), "forget-old-board-tombstones");
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
        let ports = BoardFeature::in_memory(&context);
        for idea in ["idea_1", "idea_2"] {
            ports
                .catalog
                .lay_tombstone(&IdeaLink::from(idea), NOW)
                .await
                .expect("laying a tombstone should succeed");
        }

        let forgotten = BoardTombstones::of(core(&ports, &context))
            .forget_laid_before(NOW + Duration::SECOND, 1)
            .await
            .expect("forgetting tombstones should succeed");

        assert_eq!(forgotten, 1);
    }
}
