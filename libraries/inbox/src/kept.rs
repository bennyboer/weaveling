use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Failure, Listener, ListenerName, Message, NotHandled, Subscription};
use transacting::{Transaction, Transactions};

use crate::Inbox;

#[async_trait]
pub trait KeptHandler: Send + Sync {
    const NAME: &'static str;
    const LISTENS_TO: &'static [&'static str];
    const WHEN_REFUSED: &'static str;

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure>;
}

pub struct Inboxed<H> {
    pub(crate) handler: H,
    transactions: Arc<dyn Transactions>,
    inbox: Arc<dyn Inbox>,
}

impl<H: KeptHandler> Inboxed<H> {
    pub fn new(handler: H, transactions: Arc<dyn Transactions>, inbox: Arc<dyn Inbox>) -> Self {
        Self {
            handler,
            transactions,
            inbox,
        }
    }

    async fn hear(&self, listener: &ListenerName, message: &Message) -> Result<(), Failure> {
        let mut transaction = self.transactions.begin().await?;
        if self
            .inbox
            .heard_before(&mut transaction, listener, message.id)
            .await?
        {
            return Ok(());
        }
        self.handler.take_in(&mut transaction, message).await?;

        Ok(transaction.finish().await?)
    }
}

#[async_trait]
impl<H: KeptHandler> Listener for Inboxed<H> {
    fn named(&self) -> ListenerName {
        ListenerName::parse(H::NAME).expect("a handler's name is written at compile time")
    }

    fn listens_to(&self) -> Vec<Subscription> {
        H::LISTENS_TO
            .iter()
            .map(|pattern| {
                Subscription::parse(pattern)
                    .expect("a handler's patterns are written at compile time")
            })
            .collect()
    }

    fn when_refused(&self) -> &'static str {
        H::WHEN_REFUSED
    }

    async fn handle(&self, message: &Message) -> Result<(), NotHandled> {
        let listener = self.named();

        self.hear(&listener, message)
            .await
            .map_err(|because| NotHandled {
                listener,
                routing: message.routing.clone(),
                because,
            })
    }
}
