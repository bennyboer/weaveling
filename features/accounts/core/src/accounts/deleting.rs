use eventsourcing::{Agent, AggregateId, Version};
use time::OffsetDateTime;
use transacting::Transaction;

use super::{AccountsCore, SignedIn};
use crate::account::{AccountCommand, KIND};
use crate::catalog::InProject;
use crate::deletion::{DeletionAskedBy, KEPT_BEFORE_DELETION, PendingDeletion};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::secrets::Dismissal;
use crate::session::{SessionToken, Stage};

impl AccountsCore {
    /// Asks for one's own account to be deleted, which needs the account's password.
    ///
    /// The account is kept until the returned date, so its holder can still decide to keep it.
    /// Every session of the account ends at once.
    /// An operator stops being an operator, but the last operator cannot ask for deletion.
    /// If the deletion was already asked for, the first request stands.
    pub async fn ask_deletion(
        &self,
        account: &AccountId,
        password: &str,
    ) -> Result<PendingDeletion, AccountsError> {
        self.password_confirmed(account, password).await?;
        self.nothing_to_hand_over(account).await?;
        let until = self.clock.now() + KEPT_BEFORE_DELETION;

        let pending = transacting::retrying!(&*self.transactions, |transaction| {
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

        Ok(pending)
    }

    /// Marks another account for deletion, as an operator.
    /// The account is kept for the same thirty days.
    /// Only an operator can call this deletion off.
    pub async fn mark_for_deletion(
        &self,
        by: &AccountId,
        account: &AccountId,
        expected: Option<Version>,
    ) -> Result<PendingDeletion, AccountsError> {
        self.nothing_to_hand_over(account).await?;
        let until = self.clock.now() + KEPT_BEFORE_DELETION;

        let pending = transacting::retrying!(
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

        Ok(pending)
    }

    /// Keeps one's own account after all, if its holder asked for the deletion.
    ///
    /// It is called from the session that signing in opened while the deletion was pending.
    /// That session continues signed in, under a new token.
    /// The operator role, given up when the deletion was asked for, does not come back.
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

    /// Keeps an account whose deletion was asked for, as an operator, no matter who asked.
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

    /// Remembers what an account is in a project, as the project's members report it.
    /// It is none if the account is no longer in the project.
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
                .remember_holding(
                    transaction,
                    project,
                    account,
                    version,
                    held,
                    self.clock.now(),
                )
                .await?;

            Ok(())
        })
    }

    /// Remembers that a project's members were disbanded.
    /// The members report this once the project is deleted.
    pub async fn remember_disbanded(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        version: Version,
    ) -> Result<(), AccountsError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_disbanded(transaction, project, version, self.clock.now())
                .await?;

            Ok(())
        })
    }

    /// Forgets the tombstones of disbanded projects and removed members, laid before the given time.
    /// They only guard against late messages about those projects and members.
    /// Once no such message can be retried anymore, they are no longer needed.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok::<_, AccountsError>(forgotten)
        })
    }

    /// Forgets sessions and codes that expired before the given time, at most `at_most` of each.
    /// Returns how many sessions and how many codes were forgotten.
    pub async fn forget_run_out_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<(u64, u64), AccountsError> {
        let sessions = self
            .sessions
            .forget_sessions_run_out_before(before, at_most)
            .await?;
        let codes = self
            .verifications
            .forget_codes_run_out_before(before, at_most)
            .await?;

        Ok((sessions, codes))
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
    ) -> Result<PendingDeletion, AccountsError> {
        if let Some(pending) = self.get(account).await?.state.pending_deletion() {
            return Ok(pending);
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

        self.trash
            .put_into(transaction, KIND.as_str(), &account.to_string(), until)
            .await?;

        Ok(PendingDeletion { until, asked_by })
    }

    /// Deletes an account for good, once its thirty days are over.
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
            self.events
                .forget_published(transaction, &aggregate)
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
