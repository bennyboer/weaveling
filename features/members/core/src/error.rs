use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;
use crate::members::MembersError;

#[derive(Debug, Error)]
pub enum MembersCoreError {
    #[error(transparent)]
    Events(#[from] ServiceError<MembersError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("only an owner of the project may do this")]
    NotAnOwner,
    #[error("this account is to be deleted, so its role stays as it is")]
    Leaving,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for MembersCoreError {
    fn lost_race(&self) -> bool {
        matches!(self, MembersCoreError::Events(failure) if failure.lost_race())
    }
}
