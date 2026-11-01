use eventsourcing::AggregateId;
use mailing::Mail;

use super::AccountsCore;
use crate::account::AccountCommand;
use crate::code::{CodePurpose, PendingVerification};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::password::Password;
use crate::secrets::Attempt;
use email::EmailAddress;

impl AccountsCore {
    pub async fn send_code_again(&self, email: &str) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        self.counted_mail_to(&email).await?;
        let Some((id, account)) = self.whose(&email).await? else {
            return Ok(());
        };
        if account.is_verified() {
            return Ok(());
        }

        self.mail_code(&id, &email, CodePurpose::ConfirmAddress)
            .await
    }

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        let Some(id) = self.email_addresses.holder(&email).await? else {
            return Err(AccountsError::CodeWrong);
        };
        self.checked_code(&id, CodePurpose::ConfirmAddress, code)
            .await?;

        self.commanded(&id, AccountCommand::Verify, &id.acting())
            .await?;
        self.verifications
            .close(&id, CodePurpose::ConfirmAddress)
            .await?;

        Ok(())
    }

    pub async fn ask_to_reset_password(&self, email: &str) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        self.counted_mail_to(&email).await?;
        let Some((id, _account)) = self.whose(&email).await? else {
            return Ok(());
        };

        self.mail_code(&id, &email, CodePurpose::ResetPassword)
            .await
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        let hashed = Password::new(password)?.hashed();
        let Some((id, _account)) = self.whose(&email).await? else {
            return Err(AccountsError::CodeWrong);
        };
        self.checked_code(&id, CodePurpose::ResetPassword, code)
            .await?;

        let reset: Result<(), AccountsError> =
            transacting::retrying!(&*self.transactions, |transaction| {
                self.events
                    .execute(
                        Some(&mut *transaction),
                        &AggregateId::from(&id),
                        AccountCommand::ResetPassword,
                        &id.acting(),
                    )
                    .await?;
                self.passwords.keep(transaction, &id, &hashed).await?;
                Ok(())
            });
        reset?;
        self.verifications
            .close(&id, CodePurpose::ResetPassword)
            .await?;
        self.check_sessions();

        Ok(())
    }

    pub(super) async fn checked_code(
        &self,
        id: &AccountId,
        purpose: CodePurpose,
        code: &str,
    ) -> Result<(), AccountsError> {
        let pending = match self
            .verifications
            .attempt(id, purpose, PendingVerification::MAX_TRIES)
            .await?
        {
            Attempt::Nothing => return Err(AccountsError::CodeWrong),
            Attempt::Exhausted => return Err(AccountsError::CodeExhausted),
            Attempt::Counted(pending) => pending,
        };

        if self.clock.now() > pending.until {
            return Err(AccountsError::CodeExpired);
        }
        if !pending.code.is(code) {
            return Err(AccountsError::CodeWrong);
        }

        Ok(())
    }

    pub(super) async fn mail_code(
        &self,
        id: &AccountId,
        email: &EmailAddress,
        purpose: CodePurpose,
    ) -> Result<(), AccountsError> {
        let pending = PendingVerification::issued(self.clock.now());
        self.verifications.issue(id, purpose, &pending).await?;
        let code = pending.code.as_str();
        let (subject, body) = match purpose {
            CodePurpose::ConfirmAddress => (
                "Your Weaveling code",
                format!(
                    "{code} is your code to confirm this address for Weaveling. It is good for 15 minutes."
                ),
            ),
            CodePurpose::ResetPassword => (
                "Setting a new Weaveling password",
                format!(
                    "{code} is your code to set a new password for Weaveling. It is good for 15 minutes. If you did not ask for it, you can ignore this mail, and your password stays as it is."
                ),
            ),
        };

        Ok(self
            .mailer
            .send(Mail {
                to: email.to_string(),
                subject: subject.to_owned(),
                body,
            })
            .await?)
    }
}
