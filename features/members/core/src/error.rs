use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use thiserror::Error;

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
}
