use access::AccessError;
use authorizing::AuthorizerError;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;

#[derive(Debug, Error)]
pub enum ReadingsError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    Access(#[from] AccessError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for ReadingsError {
    fn lost_race(&self) -> bool {
        matches!(self, ReadingsError::Transaction(failure) if failure.lost_race())
    }
}
