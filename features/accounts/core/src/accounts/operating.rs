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
        self.operators.keep_code(&code.digest()).await?;

        Ok(Some(code))
    }

    pub async fn become_operator(&self, id: &AccountId, code: &str) -> Result<(), AccountsError> {
        if !self.get(id).await?.state.is_verified() {
            return Err(AccountsError::NotVerifiedYet);
        }
        if !self
            .operators
            .take_code(&OperatorCode::presented(code).digest())
            .await?
        {
            return Err(AccountsError::OperatorCodeWrong);
        }

        self.commanded(id, AccountCommand::BecomeOperator, &id.acting())
            .await?;
        self.operators.appoint(id).await?;

        Ok(())
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

        self.commanded(account, AccountCommand::BecomeOperator, &by.acting())
            .await?;
        self.operators.appoint(account).await?;

        Ok(())
    }

    pub async fn dismiss_operator(
        &self,
        by: &AccountId,
        account: &AccountId,
    ) -> Result<(), AccountsError> {
        self.operating(by).await?;

        if self.operators.dismiss(account).await? == Dismissal::LastOne {
            return Err(AccountsError::LastOperator);
        }

        self.commanded(account, AccountCommand::StopBeingOperator, &by.acting())
            .await?;

        Ok(())
    }

    pub(super) async fn operating(&self, by: &AccountId) -> Result<(), AccountsError> {
        if self.get(by).await?.state.is_operator() {
            Ok(())
        } else {
            Err(AccountsError::NotAnOperator)
        }
    }
}
