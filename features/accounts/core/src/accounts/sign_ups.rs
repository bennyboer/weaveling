use eventsourcing::AggregateId;
use mailing::Mail;

use super::AccountsCore;
use crate::account::AccountCommand;
use crate::code::CodePurpose;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::name::AccountName;
use crate::operator::OperatorCode;
use crate::password::Password;
use crate::session::SessionToken;
use crate::signing_up::SigningUp;
use crate::ticket::InvitationToken;
use email::EmailAddress;

use super::sessions::SignedIn;

impl AccountsCore {
    pub async fn sign_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
        if self.signing_up == SigningUp::ByInvitation {
            return Err(AccountsError::SigningUpByInvitationOnly);
        }

        self.signed_up(email, name, password).await
    }

    pub async fn sign_up_to_operate(
        &self,
        email: &str,
        name: &str,
        password: &str,
        code: &str,
    ) -> Result<(), AccountsError> {
        if self.signing_up == SigningUp::ByInvitation {
            if !self.operator_wanted().await? {
                return Err(AccountsError::SigningUpByInvitationOnly);
            }
            if !self
                .operators
                .holds_code(&OperatorCode::presented(code).digest())
                .await?
            {
                return Err(AccountsError::OperatorCodeWrong);
            }
        }

        self.signed_up(email, name, password).await
    }

    pub async fn operator_wanted(&self) -> Result<bool, AccountsError> {
        Ok(!self.operators.any_appointed().await?)
    }

    pub(super) async fn signed_up(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<(), AccountsError> {
        let email = EmailAddress::new(email)?;
        let name = AccountName::new(name)?;
        let password = Password::new(password)?;
        self.counted_mail_to(&email).await?;

        let hashed = password.hashed();
        let id = AccountId::generate(self.clock.now());
        let mut transaction = self.transactions.begin().await?;
        if !self.claimed(&mut transaction, &email, &id).await? {
            drop(transaction);
            return self.tell_already_signed_up(&email).await;
        }
        self.events
            .begin(
                &mut transaction,
                &AggregateId::from(&id),
                AccountCommand::SignUp(email.clone(), name),
                &id.acting(),
            )
            .await?;
        self.passwords.keep(&mut transaction, &id, &hashed).await?;
        transaction.finish().await?;

        self.mail_code(&id, &email, CodePurpose::ConfirmAddress)
            .await
    }

    pub async fn sign_up_by_invitation(
        &self,
        token: &str,
        name: &str,
        password: &str,
    ) -> Result<(SessionToken, SignedIn), AccountsError> {
        let name = AccountName::new(name)?;
        let hashed = Password::new(password)?.hashed();
        let digest = InvitationToken::presented(token).digest();
        let Some(found) = self.invitation_tickets.find_ticket(&digest).await? else {
            return Err(AccountsError::InvitationInvalid);
        };
        if self.whose(&found.to).await?.is_some() {
            return Err(AccountsError::EmailAddressHasAccount);
        }
        if self.signing_up == SigningUp::ByInvitation && !found.by_an_operator {
            return Err(AccountsError::InvitationNotByAnOperator);
        }
        let mut transaction = self.transactions.begin().await?;
        let Some(ticket) = self
            .invitation_tickets
            .take_ticket(&mut transaction, &digest)
            .await?
        else {
            return Err(AccountsError::InvitationInvalid);
        };
        if self.clock.now() > ticket.until {
            return Err(AccountsError::InvitationExpired);
        }

        let id = AccountId::generate(self.clock.now());
        if !self.claimed(&mut transaction, &ticket.to, &id).await? {
            return Err(AccountsError::EmailAddressHasAccount);
        }
        self.events
            .begin(
                &mut transaction,
                &AggregateId::from(&id),
                AccountCommand::SignUpByInvitation {
                    email: ticket.to.clone(),
                    name,
                    invitation: ticket.invitation,
                },
                &id.acting(),
            )
            .await?;
        self.passwords.keep(&mut transaction, &id, &hashed).await?;
        transaction.finish().await?;
        let account = self.get(&id).await?.state;

        self.opened(id, account, self.clock.now()).await
    }

    pub(super) async fn tell_already_signed_up(
        &self,
        email: &EmailAddress,
    ) -> Result<(), AccountsError> {
        Ok(self
            .mailer
            .send(Mail {
                to: email.to_string(),
                subject: "Signing up to Weaveling".to_owned(),
                body: "Someone tried to sign up to Weaveling with this address, which already has an account. If it was you, sign in instead; if not, you can ignore this mail.".to_owned(),
            })
            .await?)
    }
}
