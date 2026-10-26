use eventsourcing::ServiceError;
use mailing::Unsent;
use thiserror::Error;
use throttling::ThrottlesError;
use time::Duration;

use crate::account::AccountError;
use crate::catalog::CatalogError;
use crate::lock::InvalidLockReason;
use crate::name::InvalidAccountName;
use crate::password::InvalidPassword;
use crate::secrets::SecretsError;
use email::{EmailAddress, InvalidEmailAddress};

#[derive(Debug, Error)]
pub enum AccountsError {
    #[error(transparent)]
    InvalidEmail(#[from] InvalidEmailAddress),
    #[error(transparent)]
    InvalidName(#[from] InvalidAccountName),
    #[error(transparent)]
    InvalidPassword(#[from] InvalidPassword),
    #[error("{0}")]
    InvalidLockReason(InvalidLockReason),
    #[error("this is not the code that was sent")]
    CodeWrong,
    #[error("this code has run out; ask for a new one")]
    CodeExpired,
    #[error("this code was tried too often; ask for a new one")]
    CodeExhausted,
    #[error("this address and password do not sign in")]
    SignInRefused,
    #[error("this address is not verified yet")]
    NotVerifiedYet,
    #[error("this account is locked")]
    Locked,
    #[error("only an operator may do this")]
    NotAnOperator,
    #[error("the last operator keeps the role")]
    LastOperator,
    #[error("this is not the code that makes an operator")]
    OperatorCodeWrong,
    #[error("signing up here is by invitation only")]
    SigningUpByInvitationOnly,
    #[error("this invitation's link lets nobody sign up, or not any more")]
    InvitationInvalid,
    #[error("this invitation has run out; ask for a new one")]
    InvitationExpired,
    #[error("this invitation was sent to {to}; sign in with that address to take it up")]
    InvitationForAnotherAddress { to: EmailAddress },
    #[error(
        "signing up here takes an operator's invitation, and this one came from a project's owner; ask an operator to invite you"
    )]
    InvitationNotByAnOperator,
    #[error("this address already has an account; sign in instead")]
    EmailAddressHasAccount,
    #[error("too many tries; wait {wait} before trying again")]
    Throttled { wait: Duration },
    #[error(transparent)]
    Events(#[from] ServiceError<AccountError>),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Throttles(#[from] ThrottlesError),
    #[error(transparent)]
    Unsent(#[from] Unsent),
}
