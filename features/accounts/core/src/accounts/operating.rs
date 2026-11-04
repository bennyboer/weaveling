use super::AccountsCore;
use crate::account::AccountCommand;
use crate::catalog::AccountSummary;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::lock::{LockKind, LockReason};
use crate::operator::OperatorCode;
use crate::secrets::Dismissal;

impl AccountsCore {
    pub async fn draw_operator_code(&self) -> Result<Option<OperatorCode>, AccountsError> {
        if self.operators.any_appointed().await? {
            return Ok(None);
        }
        let code = OperatorCode::drawn();
        let mut transaction = self.transactions.begin().await?;
        self.operators
            .keep_code(&mut transaction, &code.digest())
            .await?;
        transaction.finish().await?;

        Ok(Some(code))
    }

    pub async fn become_operator(&self, id: &AccountId, code: &str) -> Result<(), AccountsError> {
        if !self.get(id).await?.state.is_verified() {
            return Err(AccountsError::NotVerifiedYet);
        }
        let digest = OperatorCode::presented(code).digest();

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self.operators.take_code(transaction, &digest).await? {
                return Err(AccountsError::OperatorCodeWrong);
            }
            self.operators.appoint(transaction, id).await?;
            self.commanded_in(
                transaction,
                id,
                AccountCommand::BecomeOperator,
                &id.acting(),
            )
            .await
        })
    }

    pub async fn lock(
        &self,
        by: &AccountId,
        account: &AccountId,
        reason: &str,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        let reason = LockReason::new(reason).map_err(AccountsError::InvalidLockReason)?;

        self.commanded(
            account,
            AccountCommand::Lock(LockKind::Operator, reason),
            &by.acting(),
        )
        .await?;
        self.check_sessions();

        Ok(())
    }

    pub async fn unlock(&self, by: &AccountId, account: &AccountId) -> Result<(), AccountsError> {
        self.operating(by).await?;

        self.commanded(
            account,
            AccountCommand::Unlock(LockKind::Operator),
            &by.acting(),
        )
        .await?;

        Ok(())
    }

    pub async fn list(
        &self,
        by: &AccountId,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, AccountsError> {
        self.operating(by).await?;

        Ok(self.catalog.listed_after(after, at_most).await?)
    }

    pub async fn appoint_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.operators.appoint(transaction, account).await?;
            self.commanded_in(
                transaction,
                account,
                AccountCommand::BecomeOperator,
                &by.acting(),
            )
            .await
        })
    }

    pub async fn dismiss_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            if self.operators.dismiss(transaction, account).await? == Dismissal::LastOne {
                return Err(AccountsError::LastOperator);
            }
            self.commanded_in(
                transaction,
                account,
                AccountCommand::StopBeingOperator,
                &by.acting(),
            )
            .await
        })
    }

    pub(super) async fn operating(&self, by: &AccountId) -> Result<(), AccountsError> {
        if self.get(by).await?.state.is_operator() {
            Ok(())
        } else {
            Err(AccountsError::NotAnOperator)
        }
    }
}
