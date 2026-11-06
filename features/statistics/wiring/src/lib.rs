use std::sync::Arc;

use chores::ForgetOldTombstones;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::Broadcaster;
use outbox::Outbox;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{StatisticsCore, WordCountCatalog};
use statistics_messaging::ProjectWordCountsListener;
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn WordCountCatalog>,
    pub inbox: Arc<dyn Inbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn WordCountCatalog> = Arc::new(statistics_rest::TellingWordCounts::around(
        ports.catalog.clone(),
        broadcaster.clone(),
    ));

    let statistics = StatisticsCore::new(
        ports.transactions.clone(),
        telling,
        context.authorizer.clone(),
    );

    Wired::serving(statistics_rest::router(statistics.clone()))
        .followed_live(vec![statistics_rest::topic(
            statistics.clone(),
            broadcaster,
        )])
        .listening(vec![Arc::new(
            hearing.inboxed(ProjectWordCountsListener::new(statistics.clone())),
        )])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-statistics-tombstones",
                chores::tombstones_by(move |before, at_most| {
                    let statistics = statistics.clone();
                    async move {
                        statistics
                            .forget_tombstones_laid_before(before, at_most)
                            .await
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-statistics-hearings", context.clock.clone())),
        ])
}

pub struct StatisticsFeature;

impl Feature for StatisticsFeature {
    const NAME: &'static str = "statistics";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        Ports {
            transactions: Arc::new(InMemoryTransactions::new()),
            catalog: Arc::new(InMemoryWordCountCatalog::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::PostgresTransactions::new(pool.clone())),
            catalog: Arc::new(statistics_catalog::PostgresWordCountCatalog::new(pool)),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            statistics_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::SqliteTransactions::new(pool.clone())),
            catalog: Arc::new(statistics_catalog::SqliteWordCountCatalog::new(pool)),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            statistics_catalog::sqlite::migrations(),
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

    use authorizing::AllowingEverything;
    use chores::TOMBSTONES_KEPT_FOR;
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use statistics_core::SceneLink;
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_and_hearings_as_chores() {
        // given: the feature wired
        let context = Context {
            clock: Arc::new(FixedClock::new(
                NOW + TOMBSTONES_KEPT_FOR + Duration::SECOND,
            )),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
            authorizer: Arc::new(AllowingEverything),
        };
        let ports = StatisticsFeature::in_memory(&context);
        let wired = StatisticsFeature::wire(&ports, &context);
        // and: one tombstone kept long enough and one not
        for (scene, laid) in [
            ("scene_long_gone", NOW),
            ("scene_just_gone", NOW + Duration::SECOND),
        ] {
            let mut transaction = ports
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            ports
                .catalog
                .forget_scene(&mut transaction, &SceneLink::from(scene), laid)
                .await
                .expect("forgetting should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        // when: its chores are looked at
        let [tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        // then: one forgets tombstones and one hearings
        assert_eq!(tombstones.named(), "forget-old-statistics-tombstones");
        assert_eq!(hearings.named(), "forget-old-statistics-hearings");
        // and: the tombstone one forgets only the tombstone kept long enough
        assert_eq!(
            tombstones
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }
}
