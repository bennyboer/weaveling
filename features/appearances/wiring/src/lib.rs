use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{AppearanceCatalog, AppearancesCore};
use appearances_messaging::{
    ForgetDeletedProjectListener, ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener,
    ProjectSceneAppearancesListener, RememberIdeaProjectListener,
};
use chores::ForgetOldTombstones;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::Broadcaster;
use outbox::Outbox;
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn AppearanceCatalog>,
    pub inbox: Arc<dyn Inbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
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
            Arc::new(hearing.inboxed(ProjectOutlineAppearancesListener::new(appearances.clone()))),
            Arc::new(hearing.inboxed(ProjectSceneAppearancesListener::new(appearances.clone()))),
            Arc::new(hearing.inboxed(ForgetDiscardedIdeaListener::new(appearances.clone()))),
            Arc::new(hearing.inboxed(RememberIdeaProjectListener::new(appearances.clone()))),
            Arc::new(hearing.inboxed(ForgetDeletedProjectListener::new(appearances.clone()))),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-appearance-tombstones",
                chores::tombstones_by(move |before, at_most| {
                    let appearances = appearances.clone();
                    async move {
                        appearances
                            .forget_tombstones_laid_before(before, at_most)
                            .await
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-appearance-hearings", context.clock.clone())),
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
    use chores::TOMBSTONES_KEPT_FOR;
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_and_hearings_as_chores() {
        // given: a feature whose clock is past how long tombstones are kept
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
        // and: one subject forgotten long ago and one just now
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

        // when: its chores are handed over
        let [tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        // then: one forgets old tombstones and one old hearings
        assert_eq!(tombstones.named(), "forget-old-appearance-tombstones");
        assert_eq!(hearings.named(), "forget-old-appearance-hearings");
        // and: the tombstones chore forgets only the long gone subject
        assert_eq!(
            tombstones
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }
}
