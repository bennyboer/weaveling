use std::sync::Arc;

use chores::ForgetOldTombstones;
use feedback_catalog::InMemoryReadingCopyCatalog;
use feedback_core::{FeedbackCore, ReadingCopyCatalog};
use feedback_messaging::RememberReadingCopiesListener;
use inbox::{Hearing, InMemoryInbox, Inbox};
use outbox::Outbox;
use transacting::{InMemoryTransactions, Transactions};
use wiring::{Context, Feature, Wired};

// TODO: the event store and the outbox, once feedback is kept (5c)
pub struct Ports {
    pub copies: Arc<dyn ReadingCopyCatalog>,
    pub transactions: Arc<dyn Transactions>,
    pub inbox: Arc<dyn Inbox>,
}

pub fn wire(ports: &Ports, context: &Context) -> Wired {
    let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());
    let feedback = FeedbackCore::new(
        ports.copies.clone(),
        ports.transactions.clone(),
        context.clock.clone(),
    );

    Wired::serving(feedback_rest::router(feedback.clone()))
        .listening(vec![Arc::new(
            hearing.inboxed(RememberReadingCopiesListener::new(feedback.clone())),
        )])
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
    Ports {
        copies: Arc::new(InMemoryReadingCopyCatalog::new()),
        transactions: Arc::new(InMemoryTransactions::new()),
        inbox: Arc::new(InMemoryInbox::new(context.clock.clone())),
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

    fn outbox(_ports: &Ports) -> Option<Arc<dyn Outbox>> {
        None
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}
