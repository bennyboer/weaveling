use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::{AllowingEverything, Authorizer};
use chores::ForgetOldTombstones;
use inbox::{Hearing, InMemoryInbox, Inbox};
use outbox::Outbox;
use readings_catalog::SampleBook;
use readings_core::{OutlineCatalog, ReadingsCore, SceneTextCatalog};
use readings_messaging::{RememberRolesListener, RememberTrashListener};
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

pub struct Ports {
    pub outlines: Arc<dyn OutlineCatalog>,
    pub texts: Arc<dyn SceneTextCatalog>,
    pub transactions: Arc<dyn Transactions>,
    pub access: KeptAccess,
    pub inbox: Arc<dyn Inbox>,
}

impl Ports {
    fn kept_in(
        transactions: Arc<dyn Transactions>,
        access: KeptAccess,
        inbox: Arc<dyn Inbox>,
    ) -> Self {
        // TODO: catalogs kept from the outline's and the scenes' messages, in memory and in the databases
        Self {
            outlines: Arc::new(SampleBook),
            texts: Arc::new(SampleBook),
            transactions,
            access,
            inbox,
        }
    }
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let authorizer: Arc<dyn Authorizer> = if context.with_accounts {
        Arc::new(ports.access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    let readings = ReadingsCore::new(
        ports.outlines.clone(),
        ports.texts.clone(),
        ports.transactions.clone(),
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
    );

    Wired::serving(readings_rest::router(readings.clone()))
        .listening(vec![
            Arc::new(hearing.inboxed(RememberRolesListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(RememberTrashListener::new(readings.clone()))),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-readings-tombstones",
                chores::tombstones_by(move |before, at_most| {
                    let readings = readings.clone();
                    async move {
                        readings
                            .forget_tombstones_laid_before(before, at_most)
                            .await
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-readings-hearings", context.clock.clone())),
        ])
}

pub struct ReadingsFeature;

impl Feature for ReadingsFeature {
    const NAME: &'static str = "readings";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        Ports::kept_in(
            Arc::new(InMemoryTransactions::new()),
            KeptAccess::new(Arc::new(InMemoryAccess::new())),
            Arc::new(InMemoryInbox::new(context.clock.clone())),
        )
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::kept_in(
            Arc::new(transacting::PostgresTransactions::new(pool.clone())),
            KeptAccess::new(Arc::new(access::PostgresAccess::new(pool))),
            Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
        ))
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            inbox::postgres::migrations(),
            access::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(Ports::kept_in(
            Arc::new(transacting::SqliteTransactions::new(pool.clone())),
            KeptAccess::new(Arc::new(access::SqliteAccess::new(pool))),
            Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
        ))
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![inbox::sqlite::migrations(), access::sqlite::migrations()]
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

    use chores::TOMBSTONES_KEPT_FOR;
    use clock::FixedClock;
    use eventsourcing::{AccountId, Version};
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_forgets_its_old_tombstones_and_hearings_as_chores() {
        // given: the feature wired
        let context = Context {
            with_accounts: false,
            clock: Arc::new(FixedClock::new(
                NOW + TOMBSTONES_KEPT_FOR + Duration::SECOND,
            )),
            publisher: Arc::new(InProcessDispatcher::queueing_to(Arc::new(
                InMemoryDeliveries::new(),
            ))),
        };
        let ports = ReadingsFeature::in_memory(&context);
        let wired = ReadingsFeature::wire(&ports, &context);
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

        // when: its chores are looked at
        let [tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over two chores");
        };

        // then: one forgets the old records of who may read, and one old hearings
        assert_eq!(tombstones.named(), "forget-old-readings-tombstones");
        assert_eq!(hearings.named(), "forget-old-readings-hearings");
        // and: the tombstones chore forgets only the long gone account
        assert_eq!(
            tombstones
                .carry_out()
                .await
                .expect("the chore should be carried out"),
            1
        );
    }
}
