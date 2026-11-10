use eventsourcing::{Agent, AggregateId, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use super::{AccountsCore, SignedIn};
use crate::account::{AccountCommand, KIND};
use crate::catalog::InProject;
use crate::deletion::{DeletionAskedBy, KEPT_BEFORE_DELETION};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::secrets::Dismissal;
use crate::session::{SessionToken, Stage};

impl AccountsCore {
    /// Asks for one's own account to be deleted, for its password: it is kept until the answer, to be kept after all, every session of it ending at once; an operator stops being one, unless the last.
    pub async fn ask_deletion(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<OffsetDateTime, AccountsError> {
        self.password_confirmed(account, password).await?;
        self.nothing_to_hand_over(account).await?;
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
        self.nothing_to_hand_over(account).await?;
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

    /// Keeps one's own account after all, from the session signing in opened while its deletion was pending, where its holder asked for it: the session goes on signed in, under a new token; the operator's role, given up in asking, does not come back.
    pub async fn keep_account(
        &self,
        pending: &SessionToken,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let account = match self.signed_in(pending).await? {
            Some(signed_in) if signed_in.session.stage == Stage::DeletionPending => {
                signed_in.session.account
            }
            _ => return Err(AccountsError::NoDeletionPending),
        };

        let opened = transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                &account,
                None,
                AccountCommand::CallOffDeletion {
                    by: DeletionAskedBy::Holder,
                },
                &account.acting(),
            )
            .await?;
            self.trash
                .take_out(transaction, KIND.as_str(), &account.to_string())
                .await?;
            self.sessions.end(transaction, &pending.digest()).await?;
            let holder = self.get(&account).await?.state;

            self.session_opened_within(
                transaction,
                account,
                holder,
                Stage::SignedIn,
                self.clock.now(),
            )
            .await
        })?;
        self.check_sessions();

        Ok(opened)
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
                .await?;

                Ok::<_, AccountsError>(
                    self.trash
                        .take_out(transaction, KIND.as_str(), &account.to_string())
                        .await
                        .map(|_| ())?,
                )
            }
        )
    }

    /// Remembers what an account is in a project, none where it is gone, as the project's members tell it.
    pub async fn remember_holding(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        account: &AccountId,
        version: Version,
        held: Option<InProject>,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_holding(transaction, project, account, version, held)
                .await?;

            Ok(())
        })
    }

    /// Remembers a project's members disbanded, as its members tell it once the project is deleted.
    pub async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_disbanded(transaction, project, version)
                .await?;

            Ok(())
        })
    }

    async fn nothing_to_hand_over(&self, account: &AccountId) -> Result<(), AccountsError> {
        let due = self.catalog.projects_to_hand_over(account).await?;
        if due.is_empty() {
            Ok(())
        } else {
            Err(AccountsError::ProjectsToHandOver(due))
        }
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
        if self.get(account).await?.state.pending_deletion().is_some() {
            return Ok(());
        }
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
        .await?;

        Ok(self
            .trash
            .put_into(transaction, KIND.as_str(), &account.to_string(), until)
            .await?)
    }

    /// Sweeps an account whose thirty days are over, for good.
    pub async fn sweep(&self, account: &AccountId) -> Result<(), AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let aggregate = AggregateId::from(account);
            let email = self
                .events
                .latest_within(transaction, &aggregate)
                .await?
                .state
                .email()
                .clone();
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::Delete,
                &Agent::System,
            )
            .await?;
            self.events
                .compact(transaction, &aggregate, &Agent::System)
                .await?;
            self.email_addresses
                .release(transaction, &email, account)
                .await?;
            self.passwords
                .forget_password_of(transaction, account)
                .await?;
            self.sessions
                .end_every_session_of(transaction, account)
                .await?;
            self.verifications
                .forget_codes_of(transaction, account)
                .await?;
            self.second_factors
                .forget_second_factors_of(transaction, account)
                .await?;
            self.key_credentials
                .forget_credentials_of(transaction, account)
                .await?;
            self.trash
                .take_out(transaction, KIND.as_str(), &account.to_string())
                .await?;

            Ok::<_, AccountsError>(())
        })
    }
}
