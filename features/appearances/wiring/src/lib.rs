mod tombstones;

use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::AppearanceCatalog;
use appearances_messaging::{
    ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener, ProjectSceneAppearancesListener,
};
use appearances_rest::Readers;
use chores::ForgetOldTombstones;
use live::Broadcaster;
use outbox::Outbox;
use wiring::{Context, Feature, Wired};

use crate::tombstones::AppearanceTombstones;

pub struct Ports {
    pub catalog: Arc<dyn AppearanceCatalog>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn AppearanceCatalog> = Arc::new(
        appearances_rest::TellingAppearances::around(ports.catalog.clone(), broadcaster.clone()),
    );

    Wired::serving(
        appearances_rest::router(readers(ports, context))
            .merge(appearances_rest::live(broadcaster, readers(ports, context))),
    )
    .listening(vec![
        Arc::new(ProjectOutlineAppearancesListener::new(telling.clone())),
        Arc::new(ProjectSceneAppearancesListener::new(telling.clone())),
        Arc::new(ForgetDiscardedIdeaListener::new(telling)),
    ])
    .doing_chores(vec![Arc::new(ForgetOldTombstones::new(
        "forget-old-appearance-tombstones",
        Arc::new(AppearanceTombstones::of(ports.catalog.clone())),
        context.clock.clone(),
    ))])
}

fn readers(ports: &Ports, context: &Context) -> Readers {
    Readers {
        catalog: ports.catalog.clone(),
        authorizer: context.authorizer.clone(),
    }
}

pub struct AppearanceFeature;

impl Feature for AppearanceFeature {
    const NAME: &'static str = "appearances";

    type Ports = Ports;

    fn in_memory(_context: &Context) -> Ports {
        Ports {
            catalog: Arc::new(InMemoryAppearanceCatalog::new()),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(appearances_catalog::PostgresAppearanceCatalog::new(pool)),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![appearances_catalog::postgres::migrations()]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, _context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            catalog: Arc::new(appearances_catalog::SqliteAppearanceCatalog::new(pool)),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![appearances_catalog::sqlite::migrations()]
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
        let ports = AppearanceFeature::in_memory(&context);
        let wired = AppearanceFeature::wire(&ports, &context);
        for (idea, laid) in [
            ("idea_long_gone", NOW),
            ("idea_just_gone", NOW + Duration::SECOND),
        ] {
            ports
                .catalog
                .forget_subject(&Subject::Idea(IdeaLink::from(idea)), laid)
                .await
                .expect("forgetting should succeed");
        }

        let [chore] = wired.chores.as_slice() else {
            panic!("the feature hands over one chore");
        };

        assert_eq!(chore.named(), "forget-old-appearance-tombstones");
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
        let catalog: Arc<dyn AppearanceCatalog> = Arc::new(InMemoryAppearanceCatalog::new());
        for idea in ["idea_1", "idea_2"] {
            catalog
                .forget_subject(&Subject::Idea(IdeaLink::from(idea)), NOW)
                .await
                .expect("forgetting should succeed");
        }

        let forgotten = AppearanceTombstones::of(catalog)
            .forget_laid_before(NOW + Duration::SECOND, 1)
            .await
            .expect("forgetting tombstones should succeed");

        assert_eq!(forgotten, 1);
    }
}
