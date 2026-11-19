use access::AccessError;
use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;
use crate::reading_copy::ReadingCopyError;
use crate::reading_copy_name::InvalidReadingCopyName;

#[derive(Debug, Error)]
pub enum ReadingsError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Events(#[from] ServiceError<ReadingCopyError>),
    #[error(transparent)]
    InvalidName(#[from] InvalidReadingCopyName),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    Access(#[from] AccessError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("there is no such reading copy")]
    ReadingCopyNotThere,
    #[error("what is to be shared is not in the work, or ends before it begins")]
    OutsideTheWork,
    #[error("only whoever made a reading copy, or an owner of its project, may change it")]
    NotYours,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for ReadingsError {
    fn lost_race(&self) -> bool {
        match self {
            ReadingsError::Transaction(failure) => failure.lost_race(),
            ReadingsError::Events(failure) => failure.lost_race(),
            _ => false,
        }
    }
}
