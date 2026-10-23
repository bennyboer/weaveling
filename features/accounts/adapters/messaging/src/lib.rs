mod publishing;

pub use publishing::message_for;

use std::sync::Arc;

use accounts_contract::EVERY_ACCOUNT;
use accounts_core::{Account, AccountCatalog, AccountEvent, AccountId, AccountSummary};
use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use messaging::{Failure, Handler, Message};

pub struct CatalogueAccountsListener {
    events: EventSourcingService<Account>,
    catalog: Arc<dyn AccountCatalog>,
}

impl CatalogueAccountsListener {
    pub fn new(
        store: Arc<dyn EventStore<AccountEvent>>,
        catalog: Arc<dyn AccountCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }
}

#[async_trait]
impl Handler for CatalogueAccountsListener {
    const NAME: &'static str = "catalogue-account";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_ACCOUNT];
    const WHEN_REFUSED: &'static str = "A change to an account did not reach the list of accounts.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let id = eventpublishing::aggregate_in::<AccountId>(message)?;
        let standing = self.events.latest(&AggregateId::from(&id)).await?;

        Ok(self
            .catalog
            .remember(&AccountSummary::of(id, standing.version, &standing.state))
            .await?)
    }
}
