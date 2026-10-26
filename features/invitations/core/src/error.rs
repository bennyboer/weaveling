use authorizing::AuthorizerError;
use email::InvalidEmailAddress;
use eventsourcing::ServiceError;
use mailing::Unsent;
use thiserror::Error;
use throttling::ThrottlesError;
use time::Duration;

use crate::catalog::CatalogError;
use crate::invitation::InvitationError;

#[derive(Debug, Error)]
pub enum InvitationsError {
    #[error("only an operator may do this")]
    NotAnOperator,
    #[error("only an owner of the project may do this")]
    NotAnOwner,
    #[error("there is no such project")]
    ProjectNotThere,
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    InvalidEmail(#[from] InvalidEmailAddress),
    #[error("too many invitations to this address; wait {wait} before inviting it again")]
    Throttled { wait: Duration },
    #[error(transparent)]
    Events(#[from] ServiceError<InvitationError>),
    #[error(transparent)]
    Throttles(#[from] ThrottlesError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Unsent(#[from] Unsent),
}
