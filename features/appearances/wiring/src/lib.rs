use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{AppearanceCatalog, AppearancesCore};
use appearances_messaging::{
    ForgetDeletedProjectListener, ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener,
    ProjectSceneAppearancesListener, RememberIdeaProjectListener, RememberRolesListener,
    RememberTrashListener,
};
use authorizing::{AllowingEverything, Authorizer};
use chores::ForgetOldTombstones;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::Broadcaster;
use outbox::Outbox;
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn AppearanceCatalog>,
    pub access: KeptAccess,
    pub inbox: Arc<dyn Inbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let broadcaster = Broadcaster::new();
    let telling: Arc<dyn AppearanceCatalog> = Arc::new(
        appearances_rest::TellingAppearances::around(ports.catalog.clone(), broadcaster.clone()),
    );

    let authorizer: Arc<dyn Authorizer> = if context.with_accounts {
        Arc::new(ports.access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    let appearances = AppearancesCore::new(
        ports.transactions.clone(),
        telling,
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
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
            Arc::new(hearing.inboxed(RememberRolesListener::new(appearances.clone()))),
            Arc::new(hearing.inboxed(RememberTrashListener::new(appearances.clone()))),
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
            access: KeptAccess::new(Arc::new(InMemoryAccess::new())),
            inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::PostgresTransactions::new(pool.clone())),
            catalog: Arc::new(appearances_catalog::PostgresAppearanceCatalog::new(
                pool.clone(),
            )),
            access: KeptAccess::new(Arc::new(access::PostgresAccess::new(pool))),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            appearances_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
            access::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports {
            transactions: Arc::new(transacting::SqliteTransactions::new(pool.clone())),
            catalog: Arc::new(appearances_catalog::SqliteAppearanceCatalog::new(
                pool.clone(),
            )),
            access: KeptAccess::new(Arc::new(access::SqliteAccess::new(pool))),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            appearances_catalog::sqlite::migrations(),
            inbox::sqlite::migrations(),
            access::sqlite::migrations(),
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

    use chores::TOMBSTONES_KEPT_FOR;
    use clock::FixedClock;
    use eventsourcing::{AccountId, Version};
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_and_hearings_as_chores() {
        // given: a feature whose clock is past how long tombstones are kept
        let context = Context {
            with_accounts: false,
            clock: Arc::new(FixedClock::new(
                NOW + TOMBSTONES_KEPT_FOR + Duration::SECOND,
            )),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
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
        // and: one account that left long ago and one just now
        for (account, laid) in [
            ("account_long_gone", NOW),
            ("account_just_gone", NOW + Duration::SECOND),
        ] {
            let mut transaction = ports
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            ports
                .access
                .remember_role(
                    &mut transaction,
                    "project_loom",
                    &AccountId::from(account),
                    Version::of(2),
                    None,
                    laid,
                )
                .await
                .expect("remembering should succeed");
            transaction
                .finish()
                .await
                .expect("finishing should succeed");
        }

        // when: its chores are handed over
        let [tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        // then: one forgets old tombstones, its own and those of who may read, and one old hearings
        assert_eq!(tombstones.named(), "forget-old-appearance-tombstones");
        assert_eq!(hearings.named(), "forget-old-appearance-hearings");
        // and: the tombstones chore forgets only the long gone subject and the long gone account
        assert_eq!(
            tombstones
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            2
        );
    }
}
