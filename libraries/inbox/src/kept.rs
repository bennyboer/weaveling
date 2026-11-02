use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Listener, ListenerName, Message, NotHandled, Subscription};
use transacting::{LostRace, Transaction, Transactions};

use crate::Inbox;

pub struct Refused {
    lost_race: bool,
    because: Box<dyn Error + Send + Sync>,
}

impl Refused {
    pub fn of(failure: impl Error + LostRace + Send + Sync + 'static) -> Self {
        Self {
            lost_race: failure.lost_race(),
            because: Box::new(failure),
        }
    }
}

impl<E: Into<Box<dyn Error + Send + Sync>>> From<E> for Refused {
    fn from(failure: E) -> Self {
        Self {
            lost_race: false,
            because: failure.into(),
        }
    }
}

impl LostRace for Refused {
    fn lost_race(&self) -> bool {
        self.lost_race
    }
}

#[async_trait]
pub trait KeptHandler: Send + Sync {
    const NAME: &'static str;
    const LISTENS_TO: &'static [&'static str];
    const WHEN_REFUSED: &'static str;

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused>;
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

        let taken: Result<(), Refused> =
            transacting::retrying!(&*self.transactions, |transaction| {
                if self
                    .inbox
                    .heard_before(transaction, &listener, message.id)
                    .await?
                {
                    return Ok(());
                }
                self.handler.take_in(transaction, message).await
            });

        taken.map_err(|refused| NotHandled {
            listener,
            routing: message.routing.clone(),
            because: refused.because,
        })
    }
}
