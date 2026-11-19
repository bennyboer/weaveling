use eventsourcing::AccountId;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::error::ReadingsError;
use crate::readings::ReadingsCore;

impl ReadingsCore {
    /// Keeps the name an account signed up with, to name the copies it makes.
    pub async fn name_heard(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        name: &str,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.names.remember_name(transaction, account, name).await?;

            Ok(())
        })
    }

    /// Forgets a deleted account's name; the copies it made stay, as they belong to their project.
    pub async fn account_gone(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), ReadingsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.names.forget_name(transaction, account, at).await?;

            Ok(())
        })
    }
}
