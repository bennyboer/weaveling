mod tombstones;

use std::sync::Arc;

use chores::ForgetOldTombstones;
use live::Broadcaster;
use outbox::Outbox;
use statistics_catalog::InMemoryWordCountCatalog;
use statistics_core::{StatisticsCore, WordCountCatalog};
use statistics_messaging::ProjectWordCountsListener;
use wiring::{Context, Feature, Wired};

use crate::tombstones::StatisticsTombstones;

pub struct Ports {
    pub catalog: Arc<dyn WordCountCatalog>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn WordCountCatalog> = Arc::new(statistics_rest::TellingWordCounts::around(
        ports.catalog.clone(),
        broadcaster.clone(),
    ));

    let statistics = StatisticsCore::new(telling, context.authorizer.clone());

    Wired::serving(statistics_rest::router(statistics.clone()))
        .followed_live(vec![statistics_rest::topic(
            statistics.clone(),
            broadcaster,
        )])
        .listening(vec![Arc::new(ProjectWordCountsListener::new(
            statistics.clone(),
        ))])
        .doing_chores(vec![Arc::new(ForgetOldTombstones::new(
            "forget-old-statistics-tombstones",
            Arc::new(StatisticsTombstones::of(statistics)),
            context.clock.clone(),
        ))])
}

pub struct StatisticsFeature;

impl Feature for StatisticsFeature {
    const NAME: &'static str = "statistics";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports {
            catalog: Arc::new(InMemoryWordCountCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(statistics_catalog::PostgresWordCountCatalog::new(pool)),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![statistics_catalog::postgres::migrations()]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(statistics_catalog::SqliteWordCountCatalog::new(pool)),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![statistics_catalog::sqlite::migrations()]
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
    use chores::{TOMBSTONES_KEPT_FOR, Tombstones};
    use clock::FixedClock;
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use statistics_core::SceneLink;
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_as_a_chore() {
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
        for (scene, laid) in [
            ("scene_long_gone", NOW),
            ("scene_just_gone", NOW + Duration::SECOND),
        ] {
            ports
                .catalog
                .forget_scene(&SceneLink::from(scene), laid)
                .await
                .expect("forgetting should succeed");
        }

        let [chore] = wired.chores.as_slice() else {
            panic!("the feature hands over one chore");
        };

        assert_eq!(chore.named(), "forget-old-statistics-tombstones");
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
        let catalog: Arc<dyn WordCountCatalog> = Arc::new(InMemoryWordCountCatalog::new());
        for scene in ["scene_1", "scene_2"] {
            catalog
                .forget_scene(&SceneLink::from(scene), NOW)
                .await
                .expect("forgetting should succeed");
        }

        let forgotten =
            StatisticsTombstones::of(StatisticsCore::new(catalog, Arc::new(AllowingEverything)))
                .forget_laid_before(NOW + Duration::SECOND, 1)
                .await
                .expect("forgetting tombstones should succeed");

        assert_eq!(forgotten, 1);
    }
}
