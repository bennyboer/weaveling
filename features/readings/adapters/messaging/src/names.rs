use accounts_contract::{AccountEventDTO, DELETED, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use eventsourcing::AccountId;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use readings_core::ReadingsCore;
use transacting::Transaction;

pub struct RememberAccountNamesListener {
    readings: ReadingsCore,
}

impl RememberAccountNamesListener {
    pub fn new(readings: ReadingsCore) -> Self {
        Self { readings }
    }
}

#[async_trait::async_trait]
impl KeptHandler for RememberAccountNamesListener {
    const NAME: &'static str = "readings-remember-account-names";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP, SIGNED_UP_BY_INVITATION, DELETED];
    const WHEN_REFUSED: &'static str = "An account's sign-up or deletion did not reach the readings feature. Its reading copies may name no maker, or still name a deleted account.";

    /// Keeps the name an account signed up with, or forgets it once the account is deleted.
    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let account = AccountId::from(told.aggregate.id);

        match told.event.body {
            AccountEventDTO::SignedUp { personal }
            | AccountEventDTO::SignedUpByInvitation { personal } => Ok(self
                .readings
                .name_heard(Some(transaction), &account, &personal.name)
                .await?),
            AccountEventDTO::Deleted => Ok(self
                .readings
                .account_gone(Some(transaction), &account, message.occurred_at)
                .await?),
            _ => Err("this tells neither a sign-up nor an account deleted".into()),
        }
    }
}
