use std::sync::Arc;

use access::{InMemoryAccess, KeptAccess};
use authorizing::Authorizer;
use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use feedback_catalog::{
    InMemoryAccountNameCatalog, InMemoryFeedbackCatalog, InMemoryReadingCopyCatalog,
};
use feedback_core::{
    AccountNameCatalog, FeedbackCatalog, FeedbackCore, FeedbackEvent, ReadingCopyCatalog,
};
use feedback_messaging::{
    RememberAccountNamesListener, RememberReadingCopiesListener, RememberRolesListener,
    RememberStandingListener, SweepFeedbackOfDeletedReadingCopyListener,
};
use inbox::{Hearing, InMemoryInbox, Inbox};
use outbox::Outbox;
use transacting::Transactions;
use wiring::{Context, EventSourced, Feature, Wired};

pub struct Ports {
    pub feedback: Arc<dyn FeedbackCatalog>,
    pub copies: Arc<dyn ReadingCopyCatalog>,
    pub names: Arc<dyn AccountNameCatalog>,
    pub events: Arc<dyn EventStore<FeedbackEvent>>,
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
        Arc::new(ports.access.clone().without_accounts())
    };
    let feedback = FeedbackCore::new(
        ports.events.clone(),
        ports.feedback.clone(),
        ports.copies.clone(),
        ports.names.clone(),
        ports.transactions.clone(),
        ports.access.clone(),
        authorizer,
        context.clock.clone(),
    );

    Wired::serving(feedback_rest::router(feedback.clone()))
        .listening(vec![
            Arc::new(hearing.inboxed(RememberReadingCopiesListener::new(
                feedback.clone(),
                ports.outbox.clone(),
                context.clock.clone(),
            ))),
            Arc::new(SweepFeedbackOfDeletedReadingCopyListener::new(
                feedback.clone(),
            )),
            Arc::new(hearing.inboxed(RememberAccountNamesListener::new(feedback.clone()))),
            Arc::new(hearing.inboxed(RememberRolesListener::new(feedback.clone()))),
            Arc::new(hearing.inboxed(RememberStandingListener::new(feedback.clone()))),
        ])
        .doing_chores(vec![
            Arc::new(ForgetOldTombstones::new(
                "forget-old-feedback-tombstones",
                chores::tombstones_by(move |before, at_most| {
                    let feedback = feedback.clone();
                    async move {
                        feedback
                            .forget_tombstones_laid_before(before, at_most)
                            .await
                    }
                }),
                context.clock.clone(),
            )),
            Arc::new(hearing.forgetting("forget-old-feedback-hearings", context.clock.clone())),
        ])
}

/// What the feedback feature keeps, in memory.
fn kept_in_memory(context: &Context) -> Ports {
    let EventSourced {
        events,
        transactions,
        outbox,
    } = EventSourced::in_memory(feedback_messaging::message_for, context);

    Ports {
        feedback: Arc::new(InMemoryFeedbackCatalog::new()),
        copies: Arc::new(InMemoryReadingCopyCatalog::new()),
        names: Arc::new(InMemoryAccountNameCatalog::new()),
        events,
        transactions,
        access: KeptAccess::new(Arc::new(InMemoryAccess::new())),
        inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
        outbox,
    }
}

pub struct FeedbackFeature;

impl Feature for FeedbackFeature {
    const NAME: &'static str = "feedback";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        kept_in_memory(context)
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_postgres(
            &pool,
            feedback_store::codec(),
            feedback_messaging::message_for,
            context,
        );

        Ok(Ports {
            feedback: Arc::new(feedback_catalog::PostgresFeedbackCatalog::new(pool.clone())),
            copies: Arc::new(feedback_catalog::PostgresReadingCopyCatalog::new(
                pool.clone(),
            )),
            names: Arc::new(feedback_catalog::PostgresAccountNameCatalog::new(
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
            feedback_catalog::postgres::migrations(),
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
            feedback_store::codec(),
            feedback_messaging::message_for,
            context,
        );

        Ok(Ports {
            feedback: Arc::new(feedback_catalog::SqliteFeedbackCatalog::new(pool.clone())),
            copies: Arc::new(feedback_catalog::SqliteReadingCopyCatalog::new(
                pool.clone(),
            )),
            names: Arc::new(feedback_catalog::SqliteAccountNameCatalog::new(
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
            feedback_catalog::sqlite::migrations(),
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
