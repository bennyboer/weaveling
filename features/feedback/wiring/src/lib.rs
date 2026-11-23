use std::sync::Arc;

use chores::ForgetOldTombstones;
use eventsourcing::EventStore;
use feedback_catalog::{
    InMemoryAccountNameCatalog, InMemoryFeedbackCatalog, InMemoryReadingCopyCatalog,
};
use feedback_core::{
    AccountNameCatalog, FeedbackCatalog, FeedbackCore, FeedbackEvent, ReadingCopyCatalog,
};
use feedback_messaging::{RememberAccountNamesListener, RememberReadingCopiesListener};
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
    pub inbox: Arc<dyn Inbox>,
    pub outbox: Arc<dyn Outbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let feedback = FeedbackCore::new(
        ports.events.clone(),
        ports.feedback.clone(),
        ports.copies.clone(),
        ports.names.clone(),
        ports.transactions.clone(),
        context.clock.clone(),
    );

    Wired::serving(feedback_rest::router(feedback.clone()))
        .listening(vec![
            Arc::new(hearing.inboxed(RememberReadingCopiesListener::new(feedback.clone()))),
            Arc::new(hearing.inboxed(RememberAccountNamesListener::new(feedback.clone()))),
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
// TODO: kept on PostgreSQL and SQLite too, in the feature's own tables (5d)
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
    fn on_postgres(_pool: sqlx::PgPool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(kept_in_memory(context))
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(_pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, wiring::Unprepared> {
        Ok(kept_in_memory(context))
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
