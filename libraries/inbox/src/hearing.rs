use std::sync::Arc;

use chores::ForgetOldTombstones;
use clock::Clock;
use transacting::Transactions;

use crate::{EffectHandler, HeardMessages, Inbox, Inboxed, InboxedEffect, KeptHandler};

#[derive(Clone)]
pub struct Hearing {
    transactions: Arc<dyn Transactions>,
    inbox: Arc<dyn Inbox>,
}

impl Hearing {
    pub fn new(transactions: Arc<dyn Transactions>, inbox: Arc<dyn Inbox>) -> Self {
        Self {
            transactions,
            inbox,
        }
    }

    pub fn inboxed<H: KeptHandler>(&self, handler: H) -> Inboxed<H> {
        Inboxed::new(handler, self.transactions.clone(), self.inbox.clone())
    }

    pub fn inboxed_effect<H: EffectHandler>(&self, handler: H) -> InboxedEffect<H> {
        InboxedEffect::new(handler, self.transactions.clone(), self.inbox.clone())
    }

    pub fn forgetting(&self, named: &'static str, clock: Arc<dyn Clock>) -> ForgetOldTombstones {
        ForgetOldTombstones::new(
            named,
            Arc::new(HeardMessages::of(
                self.inbox.clone(),
                self.transactions.clone(),
            )),
            clock,
        )
    }
}
