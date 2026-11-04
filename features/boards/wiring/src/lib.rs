mod tombstones;

use std::sync::Arc;

use boards_catalog::InMemoryBoardCatalog;
use boards_contract::EVERY_BOARD;
use boards_core::{BoardCatalog, BoardEvent, BoardsCore};
use boards_messaging::{
    CatalogueBoardsListener, DiscardBoardsOnProjectDeletedListener, LetGoOfDiscardedIdeaListener,
    LetGoOfGoneIdeasListener, ProjectPinnedIdeasListener, RememberDeletedProjectsListener,
    UnpinOnDiscardListener,
};
use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use inbox::{HeardMessages, InMemoryInbox, Inbox, Inboxed};
use live::{BroadcastChangesListener, Broadcaster};
use outbox::Outbox;
use registry::{InMemoryRegistry, Registry};
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Wired};

use crate::tombstones::BoardTombstones;

pub const LIVE: &str = "broadcast-board-changes";

pub struct Ports {
    pub events: Arc<dyn EventStore<BoardEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn BoardCatalog>,
    pub registry: Arc<dyn Registry>,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn core(ports: &Ports, context: &Context) -> BoardsCore {
    BoardsCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        ports.registry.clone(),
        context.authorizer.clone(),
        context.clock.clone(),
    )
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let boards = core(ports, context);
    let catalogue = Inboxed::new(
        CatalogueBoardsListener::new(boards.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let index = Inboxed::new(
        ProjectPinnedIdeasListener::new(
            boards.clone(),
            ports.outbox.clone(),
            context.clock.clone(),
        ),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let unpin_discarded_ideas = Inboxed::new(
        UnpinOnDiscardListener::new(boards.clone(), ports.outbox.clone(), context.clock.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let let_go_everywhere = Inboxed::new(
        LetGoOfDiscardedIdeaListener::new(boards.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let let_go_of_the_gone = Inboxed::new(
        LetGoOfGoneIdeasListener::new(boards.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let remember_deleted_projects = Inboxed::new(
        RememberDeletedProjectsListener::new(
            boards.clone(),
            ports.outbox.clone(),
            context.clock.clone(),
        ),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
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
            Arc::new(let_go_everywhere),
            Arc::new(let_go_of_the_gone),
            Arc::new(remember_deleted_projects),
            Arc::new(sweep),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-board-tombstones",
                Arc::new(BoardTombstones::of(boards)),
                context.clock.clone(),
            )),
            Arc::new(ForgetOldTombstones::new(
                "forget-old-board-hearings",
                Arc::new(HeardMessages::of(
                    ports.inbox.clone(),
                    ports.transactions.clone(),
                )),
                context.clock.clone(),
            )),
        ])
}

pub struct BoardFeature;

impl Feature for BoardFeature {
    const NAME: &'static str = "boards";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(boards_messaging::message_for, context);

        Ports {
            events,
            transactions,
            catalog: Arc::new(InMemoryBoardCatalog::new()),
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
            boards_store::codec(),
            boards_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(boards_catalog::PostgresBoardCatalog::new(pool.clone())),
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
            boards_catalog::postgres::migrations(),
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
            boards_store::codec(),
            boards_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(boards_catalog::SqliteBoardCatalog::new(pool.clone())),
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
            boards_catalog::sqlite::migrations(),
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
        // given: the wired feature with a tombstone past keeping and one not
        let context = a_context();
        let ports = BoardFeature::in_memory(&context);
        let wired = BoardFeature::wire(&ports, &context);
        for (idea, laid) in [
            ("idea_long_gone", NOW),
            ("idea_just_gone", NOW + Duration::SECOND),
        ] {
            core(&ports, &context)
                .lay_tombstone(None, idea, laid)
                .await
                .expect("laying a tombstone should succeed");
        }

        // when: its chores are taken
        let [chore, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        // then: one forgets old hearings
        assert_eq!(hearings.named(), "forget-old-board-hearings");

        // and: one forgets the tombstones past keeping
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
        // given: two tombstones past keeping
        let context = a_context();
        let ports = BoardFeature::in_memory(&context);
        for idea in ["idea_1", "idea_2"] {
            core(&ports, &context)
                .lay_tombstone(None, idea, NOW)
                .await
                .expect("laying a tombstone should succeed");
        }

        // when: the tombstones are forgotten a batch of one at a time
        let forgotten = BoardTombstones::of(core(&ports, &context))
            .forget_laid_before(NOW + Duration::SECOND, 1)
            .await
            .expect("forgetting tombstones should succeed");

        // then: one is forgotten
        assert_eq!(forgotten, 1);
    }
}
