use std::error::Error;

use accounts_core::{AccountId, AccountsCore, KIND};
use async_trait::async_trait;
use trash::{Purge, Trashed};

/// The accounts' answer to an account whose thirty days are over: swept for good.
pub struct AccountsPurge {
    accounts: AccountsCore,
}

impl AccountsPurge {
    pub fn of(accounts: AccountsCore) -> Self {
        Self { accounts }
    }
}

#[async_trait]
impl Purge for AccountsPurge {
    async fn purge(&self, trashed: &Trashed) -> Result<(), Box<dyn Error + Send + Sync>> {
        if trashed.kind != KIND.as_str() {
            return Err(format!("accounts keep no {} in their trash", trashed.kind).into());
        }
        let account: AccountId = trashed
            .id
            .parse()
            .map_err(|_| format!("{} is no account's id", trashed.id))?;

        Ok(self.accounts.sweep(&account).await?)
    }
}
