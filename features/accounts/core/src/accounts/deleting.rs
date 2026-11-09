use eventsourcing::{Agent, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use super::AccountsCore;
use crate::account::AccountCommand;
use crate::deletion::{DeletionAskedBy, KEPT_BEFORE_DELETION};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::secrets::Dismissal;

impl AccountsCore {
    /// Asks for one's own account to be deleted, for its password: it is kept until the answer, to be kept after all, every session of it ending at once; an operator stops being one, unless the last.
    pub async fn ask_deletion(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<OffsetDateTime, AccountsError> {
        self.password_confirmed(account, password).await?;
        let until = self.clock.now() + KEPT_BEFORE_DELETION;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.deletion_asked_in(
                transaction,
                account,
                None,
                until,
                DeletionAskedBy::Holder,
                &account.acting(),
            )
            .await
        })?;
        self.check_sessions();

        Ok(until)
    }

    /// Marks another account for deletion, as the operator: the same thirty days, and only the operator calls it off.
    pub async fn mark_for_deletion(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<OffsetDateTime, AccountsError> {
        let until = self.clock.now() + KEPT_BEFORE_DELETION;

        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                self.deletion_asked_in(
                    transaction,
                    account,
                    expected,
                    until,
                    DeletionAskedBy::Operator,
                    &by.acting(),
                )
                .await
            }
        )?;
        self.check_sessions();

        Ok(until)
    }

    /// Keeps one's own account after all, where its holder asked for it to be deleted; the operator's role, given up in asking, does not come back.
    pub async fn keep_account(&self, account: &AccountId) -> Result<(), AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::CallOffDeletion {
                    by: DeletionAskedBy::Holder,
                },
                &account.acting(),
            )
            .await
        })
    }

    /// Keeps an account asked to be deleted, as the operator, whoever asked.
    pub async fn keep_account_of(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.still_operating(transaction, by).await?;
                self.commanded_in(
                    transaction,
                    account,
                    expected,
                    AccountCommand::CallOffDeletion {
                        by: DeletionAskedBy::Operator,
                    },
                    &by.acting(),
                )
                .await
            }
        )
    }

    async fn deletion_asked_in(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        expected: Option<Version>,
        until: OffsetDateTime,
        asked_by: DeletionAskedBy,
        agent: &Agent,
    ) -> Result<(), AccountsError> {
        if self.operators.dismiss(transaction, account).await? == Dismissal::LastOne {
            return Err(AccountsError::LastOperator);
        }

        self.commanded_in(
            transaction,
            account,
            expected,
            AccountCommand::AskDeletion {
                until,
                by: asked_by,
            },
            agent,
        )
        .await
    }
}
