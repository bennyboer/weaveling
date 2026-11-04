use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Failure, Listener, ListenerName, Message, NotHandled, Subscription};
use transacting::{Transaction, Transactions};

use crate::{Failed, Inbox};

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

#[async_trait]
pub trait EffectHandler: Send + Sync {
    const NAME: &'static str;
    const LISTENS_TO: &'static [&'static str];
    const WHEN_REFUSED: &'static str;

    async fn take_in(&self, message: &Message) -> Result<(), Failure>;
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
        transacting::retrying!(&*self.transactions, |transaction| {
            if self
                .inbox
                .heard_before(transaction, listener, message.id)
                .await
                .map_err(Failed::from)?
            {
                return Ok(());
            }
            self.handler
                .take_in(transaction, message)
                .await
                .map_err(Failed)
        })
        .map_err(|unheard| unheard.0)
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

pub struct InboxedEffect<H> {
    pub(crate) handler: H,
    transactions: Arc<dyn Transactions>,
    inbox: Arc<dyn Inbox>,
}

impl<H: EffectHandler> InboxedEffect<H> {
    pub fn new(handler: H, transactions: Arc<dyn Transactions>, inbox: Arc<dyn Inbox>) -> Self {
        Self {
            handler,
            transactions,
            inbox,
        }
    }

    async fn hear(&self, listener: &ListenerName, message: &Message) -> Result<(), Failure> {
        let heard = transacting::retrying!(&*self.transactions, |asking| {
            Ok::<_, Failed>(self.inbox.was_heard(asking, listener, message.id).await?)
        })
        .map_err(|unheard| unheard.0)?;
        if heard {
            return Ok(());
        }
        self.handler.take_in(message).await?;

        transacting::retrying!(&*self.transactions, |noting| {
            Ok::<_, Failed>(
                self.inbox
                    .heard_before(noting, listener, message.id)
                    .await?,
            )
        })
        .map(|_| ())
        .map_err(|unheard| unheard.0)
    }
}

#[async_trait]
impl<H: EffectHandler> Listener for InboxedEffect<H> {
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
