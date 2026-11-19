use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use access::{InMemoryAccess, KeptAccess};
use async_trait::async_trait;
use authorizing::{AllowingEverything, Authorizer};
use chores::{Chore, ForgetOldTombstones};
use eventsourcing::EventStore;
use inbox::{Hearing, InMemoryInbox, Inbox};
use live::Broadcaster;
use outbox::Outbox;
use readings_catalog::{
    InMemoryAccountNameCatalog, InMemoryOutlineCatalog, InMemoryReadingCopyCatalog,
    InMemoryReadingCopyContents, InMemorySceneTextCatalog,
};
use readings_core::{
    AccountNameCatalog, OutlineCatalog, ReadingCopyCatalog, ReadingCopyContents, ReadingCopyEvent,
    ReadingsCore, SceneTextCatalog,
};
use readings_messaging::{
    CatalogueReadingCopiesListener, KeepBookTitlesListener, KeepOutlinesListener,
    KeepSceneTextsListener, RememberAccountNamesListener, RememberRolesListener,
    RememberTrashListener,
};
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Wired};

/// How many copies due for deletion are deleted together, before the chore asks for more.
const DUE_EACH_BATCH: u64 = 100;

/// Once an hour, deletes the reading copies whose deletion has come, with what they hold.
/// The dates run on while a project lies in the trash, as its readers were told them.
struct DeleteDueCopies {
    readings: ReadingsCore,
}

#[async_trait]
impl Chore for DeleteDueCopies {
    fn named(&self) -> &'static str {
        "delete-due-reading-copies"
    }

    fn every(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }

    /// Asks again while a whole batch was deleted.
    /// Copies deleted already stay listed until their deletion is heard, so a batch of those ends it rather than asking for them again.
    async fn carry_out(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        let mut deleted = 0;

        loop {
            let batch = self.readings.delete_due_copies(DUE_EACH_BATCH).await?;
            deleted += batch;
            if batch < DUE_EACH_BATCH {
                return Ok(deleted);
            }
        }
    }
}

pub struct Ports {
    pub outlines: Arc<dyn OutlineCatalog>,
    pub texts: Arc<dyn SceneTextCatalog>,
    pub copies: Arc<dyn ReadingCopyCatalog>,
    pub names: Arc<dyn AccountNameCatalog>,
    pub reading_copy_contents: Arc<dyn ReadingCopyContents>,
    pub events: Arc<dyn EventStore<ReadingCopyEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub access: KeptAccess,
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let authorizer: Arc<dyn Authorizer> = if context.with_accounts {
        Arc::new(ports.access.clone())
    } else {
        Arc::new(AllowingEverything)
    };

    let broadcaster = Broadcaster::new();
    let readings = ReadingsCore::new(
        ports.outlines.clone(),
        ports.texts.clone(),
        ports.copies.clone(),
        ports.names.clone(),
        ports.reading_copy_contents.clone(),
        ports.events.clone(),
        Arc::new(readings_rest::TellingFollowers::new(
            ports.outlines.clone(),
            ports.texts.clone(),
            broadcaster.clone(),
        )),
        ports.transactions.clone(),
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
    );

    Wired::serving(readings_rest::router(readings.clone()))
        .followed_live(vec![readings_rest::topic(readings.clone(), broadcaster)])
        .listening(vec![
            Arc::new(hearing.inboxed(RememberRolesListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(RememberTrashListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(KeepOutlinesListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(KeepBookTitlesListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(KeepSceneTextsListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(CatalogueReadingCopiesListener::new(readings.clone()))),
            Arc::new(hearing.inboxed(RememberAccountNamesListener::new(readings.clone()))),
        ])
        .doing_chores(vec![
            Arc::new(DeleteDueCopies {
                readings: readings.clone(),
            }),
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
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(readings_messaging::message_for, context);

        Ports {
            outlines: Arc::new(InMemoryOutlineCatalog::new()),
            texts: Arc::new(InMemorySceneTextCatalog::new()),
            copies: Arc::new(InMemoryReadingCopyCatalog::new()),
            names: Arc::new(InMemoryAccountNameCatalog::new()),
            reading_copy_contents: Arc::new(InMemoryReadingCopyContents::new()),
            events,
            transactions,
            access: KeptAccess::new(Arc::new(InMemoryAccess::new())),
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
            readings_store::codec(),
            readings_messaging::message_for,
            context,
        );

        Ok(Ports {
            outlines: Arc::new(readings_catalog::PostgresOutlineCatalog::new(pool.clone())),
            texts: Arc::new(readings_catalog::PostgresSceneTextCatalog::new(
                pool.clone(),
            )),
            copies: Arc::new(readings_catalog::PostgresReadingCopyCatalog::new(
                pool.clone(),
            )),
            names: Arc::new(readings_catalog::PostgresAccountNameCatalog::new(
                pool.clone(),
            )),
            reading_copy_contents: Arc::new(readings_catalog::PostgresReadingCopyContents::new(
                pool.clone(),
            )),
            events,
            transactions,
            access: KeptAccess::new(Arc::new(access::PostgresAccess::new(pool))),
            inbox: Arc::new(inbox::PostgresInbox::new(context.clock.clone())),
            outbox,
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            readings_catalog::postgres::migrations(),
            inbox::postgres::migrations(),
            access::postgres::migrations(),
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
            readings_store::codec(),
            readings_messaging::message_for,
            context,
        );

        Ok(Ports {
            outlines: Arc::new(readings_catalog::SqliteOutlineCatalog::new(pool.clone())),
            texts: Arc::new(readings_catalog::SqliteSceneTextCatalog::new(pool.clone())),
            copies: Arc::new(readings_catalog::SqliteReadingCopyCatalog::new(
                pool.clone(),
            )),
            names: Arc::new(readings_catalog::SqliteAccountNameCatalog::new(
                pool.clone(),
            )),
            reading_copy_contents: Arc::new(readings_catalog::SqliteReadingCopyContents::new(
                pool.clone(),
            )),
            events,
            transactions,
            access: KeptAccess::new(Arc::new(access::SqliteAccess::new(pool))),
            inbox: Arc::new(inbox::SqliteInbox::new(context.clock.clone())),
            outbox,
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            readings_catalog::sqlite::migrations(),
            inbox::sqlite::migrations(),
            access::sqlite::migrations(),
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

    use chores::TOMBSTONES_KEPT_FOR;
    use clock::FixedClock;
    use eventsourcing::{AccountId, Version};
    use messaging::{InMemoryDeliveries, InProcessDispatcher};
    use time::{Duration, OffsetDateTime};

    use super::*;

    const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    #[tokio::test]
    async fn the_feature_deletes_due_copies_and_forgets_its_old_tombstones_and_hearings_as_chores()
    {
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
        let [due, tombstones, hearings] = wired.chores.as_slice() else {
            panic!("the feature hands over three chores");
        };

        // then: one deletes the copies due for deletion, one forgets the old records of who may read, and one old hearings
        assert_eq!(due.named(), "delete-due-reading-copies");
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
