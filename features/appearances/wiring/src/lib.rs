mod tombstones;

use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{AppearanceCatalog, AppearancesCore};
use appearances_messaging::{
    ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener, ProjectSceneAppearancesListener,
};
use chores::ForgetOldTombstones;
use inbox::{HeardMessages, InMemoryInbox, Inbox, Inboxed};
use live::Broadcaster;
use outbox::Outbox;
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

use crate::tombstones::AppearanceTombstones;

pub struct Ports {
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn AppearanceCatalog>,
    pub inbox: Arc<dyn Inbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn AppearanceCatalog> = Arc::new(
        appearances_rest::TellingAppearances::around(ports.catalog.clone(), broadcaster.clone()),
    );

    let appearances = AppearancesCore::new(
        ports.transactions.clone(),
        telling,
        context.authorizer.clone(),
    );

    Wired::serving(appearances_rest::router(appearances.clone()))
        .followed_live(vec![Arc::new(appearances_rest::AppearancesTopic::new(
            appearances.clone(),
            broadcaster,
        ))])
        .listening(vec![
            Arc::new(Inboxed::new(
                ProjectOutlineAppearancesListener::new(appearances.clone()),
                ports.transactions.clone(),
                ports.inbox.clone(),
            )),
            Arc::new(Inboxed::new(
                ProjectSceneAppearancesListener::new(appearances.clone()),
                ports.transactions.clone(),
                ports.inbox.clone(),
            )),
            Arc::new(Inboxed::new(
                ForgetDiscardedIdeaListener::new(appearances.clone()),
                ports.transactions.clone(),
                ports.inbox.clone(),
            )),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-appearance-tombstones",
                Arc::new(AppearanceTombstones::of(appearances)),
                context.clock.clone(),
            )),
            Arc::new(ForgetOldTombstones::new(
                "forget-old-appearance-hearings",
                Arc::new(HeardMessages::of(
                    ports.inbox.clone(),
                    ports.transactions.clone(),
                )),
                context.clock.clone(),
            )),
        ])
}

pub struct AppearanceFeature;

impl Feature for AppearanceFeature {
    const NAME: &'static str = "appearances";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        Ports {
            transactions: Arc::new(InMemoryTransactions::new()),
            catalog: Arc::new(InMemoryAppearanceCatalog::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::PostgresTransactions::new(pool.clone())),
            catalog: Arc::new(appearances_catalog::PostgresAppearanceCatalog::new(pool)),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            appearances_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::SqliteTransactions::new(pool.clone())),
            catalog: Arc::new(appearances_catalog::SqliteAppearanceCatalog::new(pool)),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            appearances_catalog::sqlite::migrations(),
            inbox::sqlite::migrations(),
        ]
    }

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use appearances_core::{IdeaLink, Subject};
    use authorizing::AllowingEverything;
    use chores::{TOMBSTONES_KEPT_FOR, Tombstones};
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_and_hearings_as_chores() {
        let context = Context {
            clock: Arc::new(FixedClock::new(
                NOW + TOMBSTONES_KEPT_FOR + Duration::SECOND,
            )),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
            authorizer: Arc::new(AllowingEverything),
        };
        let ports = AppearanceFeature::in_memory(&context);
        let wired = AppearanceFeature::wire(&ports, &context);
        for (idea, laid) in [
            ("idea_long_gone", NOW),
            ("idea_just_gone", NOW + Duration::SECOND),
        ] {
            let mut transaction = ports
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            ports
                .catalog
                .forget_subject(&mut transaction, &Subject::Idea(IdeaLink::from(idea)), laid)
                .await
                .expect("forgetting should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        let [tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        assert_eq!(tombstones.named(), "forget-old-appearance-tombstones");
        assert_eq!(hearings.named(), "forget-old-appearance-hearings");
        assert_eq!(
            tombstones
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }

    #[tokio::test]
    async fn the_tombstones_handed_to_the_chore_forget_no_more_than_a_batch() {
        let appearances = AppearancesCore::new(
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryAppearanceCatalog::new()),
            Arc::new(AllowingEverything),
        );
        for idea in ["idea_1", "idea_2"] {
            appearances
                .idea_discarded(None, idea, NOW)
                .await
                .expect("forgetting should succeed");
        }

        let forgotten = AppearanceTombstones::of(appearances)
            .forget_laid_before(NOW + Duration::SECOND, 1)
            .await
            .expect("forgetting tombstones should succeed");

        assert_eq!(forgotten, 1);
    }
}
