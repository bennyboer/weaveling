mod counting;
mod tombstones;

use std::sync::Arc;

use async_trait::async_trait;

use chores::ForgetOldTombstones;
use inbox::{HeardMessages, InMemoryInbox, Inbox, Inboxed};
use live::Broadcaster;
use outbox::{InMemoryOutbox, Outbox};
use scenes_catalog::{InMemoryQuotationRestylingTasks, InMemorySceneCatalog};
use scenes_core::{QuotationRestylingTasks, SceneCatalog, SceneStore, ScenesCore};
use scenes_messaging::{
    CatalogueScenesListener, DeleteOnProjectDeletedListener, RestyleOnQuotationChangedListener,
    UnlinkOnDiscardListener,
};
use scenes_store::InMemorySceneStore;
use scenes_sync::LiveScenes;
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Settling, Wired};

use crate::counting::CountTheUncounted;
use crate::tombstones::SceneTombstones;

pub struct Ports {
    pub store: Arc<dyn SceneStore>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn SceneCatalog>,
    pub quotation_restyling_tasks: Arc<dyn QuotationRestylingTasks>,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let scenes_told = Broadcaster::new();
    let catalog: Arc<dyn SceneCatalog> = Arc::new(scenes_rest::TellingScenes::around(
        ports.catalog.clone(),
        scenes_told.clone(),
    ));
    let restylings = Broadcaster::new();
    let tasks: Arc<dyn QuotationRestylingTasks> =
        Arc::new(scenes_rest::TellingRestylingTasks::around(
            ports.quotation_restyling_tasks.clone(),
            restylings.clone(),
        ));
    let core = ScenesCore::new(
        ports.store.clone(),
        ports.transactions.clone(),
        catalog,
        tasks,
        context.authorizer.clone(),
        context.clock.clone(),
    );

    let unlink = Inboxed::new(
        UnlinkOnDiscardListener::new(core.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let sweep = DeleteOnProjectDeletedListener::new(
        core.clone(),
        context.publisher.clone(),
        context.clock.clone(),
    );
    let catalogue = Inboxed::new(
        CatalogueScenesListener::new(core.clone()),
        ports.transactions.clone(),
        ports.inbox.clone(),
    );
    let live = LiveScenes::new(core.clone());
    let restyle = RestyleOnQuotationChangedListener::new(
        core.clone(),
        Arc::new(live.clone()),
        context.publisher.clone(),
        context.clock.clone(),
    );

    Wired::serving(
        scenes_rest::router(core.clone())
            .merge(scenes_rest::quotation_restylings(core.clone()))
            .merge(scenes_sync::router(live.clone())),
    )
    .followed_live(vec![
        scenes_rest::scenes_topic(core.clone(), scenes_told),
        scenes_rest::restylings_topic(core.clone(), restylings),
    ])
    .listening(vec![
        Arc::new(sweep),
        Arc::new(unlink),
        Arc::new(catalogue),
        Arc::new(restyle),
    ])
    .settling_before_stopping(vec![Arc::new(SettlingLiveScenes(live.clone()))])
    .doing_chores(vec![
        Arc::new(CountTheUncounted::of(live)),
        Arc::new(ForgetOldTombstones::new(
            "forget-old-scene-tombstones",
            Arc::new(SceneTombstones::of(core)),
            context.clock.clone(),
        )),
        Arc::new(ForgetOldTombstones::new(
            "forget-old-scene-hearings",
            Arc::new(HeardMessages::of(
                ports.inbox.clone(),
                ports.transactions.clone(),
            )),
            context.clock.clone(),
        )),
    ])
}

struct SettlingLiveScenes(LiveScenes);

#[async_trait]
impl Settling for SettlingLiveScenes {
    async fn settle(&self) {
        self.0.settle().await;
    }
}

pub struct SceneFeature;

impl Feature for SceneFeature {
    const NAME: &'static str = "scenes";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let outbox = Arc::new(InMemoryOutbox::new(
            context.publisher.clone(),
            context.clock.clone(),
        ));

        Ports {
            transactions: Arc::new(InMemoryTransactions::new()),
            store: Arc::new(InMemorySceneStore::new().enqueuing_to(
                outbox.clone(),
                context.clock.clone(),
                scenes_messaging::message_for,
            )),
            catalog: Arc::new(InMemorySceneCatalog::new()),
            quotation_restyling_tasks: Arc::new(InMemoryQuotationRestylingTasks::new()),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
            outbox,
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::PostgresTransactions::new(pool.clone())),
            store: Arc::new(
                scenes_store::PostgresSceneStore::new(pool.clone())
                    .enqueuing(context.clock.clone(), scenes_messaging::message_for),
            ),
            catalog: Arc::new(scenes_catalog::PostgresSceneCatalog::new(pool.clone())),
            quotation_restyling_tasks: Arc::new(
                scenes_catalog::PostgresQuotationRestylingTasks::new(pool.clone()),
            ),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
            outbox: Arc::new(outbox::PostgresOutbox::new(
                pool,
                context.publisher.clone(),
                context.clock.clone(),
            )),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            outbox::postgres::migrations(),
            scenes_store::postgres::migrations(),
            scenes_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::SqliteTransactions::new(pool.clone())),
            store: Arc::new(
                scenes_store::SqliteSceneStore::new(pool.clone())
                    .enqueuing(context.clock.clone(), scenes_messaging::message_for),
            ),
            catalog: Arc::new(scenes_catalog::SqliteSceneCatalog::new(pool.clone())),
            quotation_restyling_tasks: Arc::new(
                scenes_catalog::SqliteQuotationRestylingTasks::new(pool.clone()),
            ),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
            outbox: Arc::new(outbox::SqliteOutbox::new(
                pool,
                context.publisher.clone(),
                context.clock.clone(),
            )),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            outbox::sqlite::migrations(),
            scenes_store::sqlite::migrations(),
            scenes_catalog::sqlite::migrations(),
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
    use scenes_core::SceneId;
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
    async fn the_feature_counts_what_is_left_uncounted_and_forgets_its_old_tombstones_and_hearings_as_chores()
     {
        let context = a_context();
        let ports = SceneFeature::in_memory(&context);
        let wired = SceneFeature::wire(&ports, &context);
        for (made, laid) in [(1, NOW), (2, NOW + Duration::SECOND)] {
            let mut transaction = ports
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            ports
                .catalog
                .forget(
                    &mut transaction,
                    SceneId::generate(NOW + Duration::seconds(made)),
                    laid,
                )
                .await
                .expect("forgetting should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        let [counting, tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over three chores");
        };

        assert_eq!(counting.named(), "count-scenes-left-uncounted");
        assert_eq!(tombstones.named(), "forget-old-scene-tombstones");
        assert_eq!(hearings.named(), "forget-old-scene-hearings");
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
        let context = a_context();
        let ports = SceneFeature::in_memory(&context);
        for made in [1, 2] {
            let mut transaction = ports
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            ports
                .catalog
                .forget(
                    &mut transaction,
                    SceneId::generate(NOW + Duration::seconds(made)),
                    NOW,
                )
                .await
                .expect("forgetting should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }
        let scenes = ScenesCore::new(
            ports.store.clone(),
            ports.transactions.clone(),
            ports.catalog.clone(),
            ports.quotation_restyling_tasks.clone(),
            context.authorizer.clone(),
            context.clock.clone(),
        );

        let forgotten = SceneTombstones::of(scenes)
            .forget_laid_before(NOW + Duration::SECOND, 1)
            .await
            .expect("forgetting tombstones should succeed");

        assert_eq!(forgotten, 1);
    }
}
