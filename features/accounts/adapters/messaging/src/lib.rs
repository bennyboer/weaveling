mod publishing;
mod ticketing;

pub use publishing::message_for;
pub use ticketing::KeepInvitationTicketsListener;

use accounts_contract::EVERY_ACCOUNT;
use accounts_core::{AccountId, AccountsCore};
use async_trait::async_trait;
use messaging::{Failure, Handler, Message};

pub struct CatalogueAccountsListener {
    accounts: AccountsCore,
}

impl CatalogueAccountsListener {
    pub fn new(accounts: AccountsCore) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl Handler for CatalogueAccountsListener {
    const NAME: &'static str = "catalogue-account";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_ACCOUNT];
    const WHEN_REFUSED: &'static str = "A change to an account did not reach the list of accounts.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        Ok(self
            .accounts
            .catalogue(&eventpublishing::aggregate_in::<AccountId>(message)?)
            .await?)
    }
}
