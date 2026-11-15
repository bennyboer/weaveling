use mailing::Mail;

use super::AccountsCore;
use crate::account::AccountCommand;
use crate::code::{CodeDigest, CodePurpose, PendingVerification};
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::password::Password;
use crate::secrets::Attempt;
use crate::throttle::Guarded;
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

        self.mail_code_asked_for(&id, &email, CodePurpose::ConfirmAddress)
            .await
    }

    pub async fn verify(&self, email: &str, code: &str) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        self.admitted(
            Guarded::GuessingCodes(CodePurpose::ConfirmAddress),
            email.as_str(),
        )
        .await?;
        let Some(id) = self.email_addresses.holder(&email).await? else {
            return Err(AccountsError::CodeWrong);
        };
        let code = self
            .checked_code(&id, &email, CodePurpose::ConfirmAddress, code)
            .await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            if !self
                .verifications
                .close(transaction, &id, CodePurpose::ConfirmAddress, &code)
                .await?
            {
                return Err(AccountsError::CodeWrong);
            }
            self.commanded_in(transaction, &id, None, AccountCommand::Verify, &id.acting())
                .await?;

            Ok(())
        })
    }

    pub async fn ask_to_reset_password(&self, email: &str) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        self.counted_mail_to(&email).await?;
        let Some((id, _account)) = self.whose(&email).await? else {
            return Ok(());
        };

        self.mail_code_asked_for(&id, &email, CodePurpose::ResetPassword)
            .await
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        let password = Password::new(password)?;
        self.admitted(
            Guarded::GuessingCodes(CodePurpose::ResetPassword),
            email.as_str(),
        )
        .await?;
        let Some((id, _account)) = self.whose(&email).await? else {
            return Err(AccountsError::CodeWrong);
        };
        let code = self
            .checked_code(&id, &email, CodePurpose::ResetPassword, code)
            .await?;
        let hashed = password.hashed_aside().await;

        let reset: Result<(), AccountsError> =
            transacting::retrying!(&*self.transactions, |transaction| {
                if !self
                    .verifications
                    .close(transaction, &id, CodePurpose::ResetPassword, &code)
                    .await?
                {
                    return Err(AccountsError::CodeWrong);
                }
                self.commanded_in(
                    transaction,
                    &id,
                    None,
                    AccountCommand::ResetPassword,
                    &id.acting(),
                )
                .await?;
                self.passwords.keep(transaction, &id, &hashed).await?;
                Ok(())
            });
        reset?;
        self.check_sessions();

        Ok(())
    }

    /// Checks a code typed for the account, once its guess is admitted by the address.
    /// A code that is wrong, has run out, or was tried too often is refused the same way.
    /// So the answer tells no more than that the code does not work.
    async fn checked_code(
        &self,
        id: &AccountId,
        email: &EmailAddress,
        purpose: CodePurpose,
        code: &str,
    ) -> Result<CodeDigest, AccountsError> {
        let attempt = transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AccountsError>(
                self.verifications
                    .attempt(transaction, id, purpose, PendingVerification::MAX_TRIES)
                    .await?,
            )
        })?;
        let Attempt::Counted(pending) = attempt else {
            return Err(AccountsError::CodeWrong);
        };
        if self.clock.now() > pending.until || !pending.digest.admits(code) {
            return Err(AccountsError::CodeWrong);
        }
        self.throttles
            .clear(Guarded::GuessingCodes(purpose).as_str(), email.as_str())
            .await?;

        Ok(pending.digest)
    }

    /// Mails a code that was asked for again, or to set a new password.
    /// A mail that cannot be sent is logged, and the answer is the same as if it was sent.
    /// The answer for an address nobody has is the same too, so the answer tells nothing about the address.
    async fn mail_code_asked_for(
        &self,
        id: &AccountId,
        email: &EmailAddress,
        purpose: CodePurpose,
    ) -> Result<(), AccountsError> {
        match self.mail_code(id, email, purpose).await {
            Err(AccountsError::Unsent(unsent)) => {
                tracing::error!(%unsent, ?purpose, "a code that was asked for could not be mailed");

                Ok(())
            }
            mailed => mailed,
        }
    }

    pub(super) async fn mail_code(
        &self,
        id: &AccountId,
        email: &EmailAddress,
        purpose: CodePurpose,
    ) -> Result<(), AccountsError> {
        let (code, pending) = PendingVerification::issued(self.clock.now());
        transacting::retrying!(&*self.transactions, |transaction| {
            Ok::<_, AccountsError>(
                self.verifications
                    .issue(transaction, id, purpose, &pending)
                    .await?,
            )
        })?;
        let code = code.as_str();
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
