use authorizing::AuthorizerError;
use ids::InvalidId;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::{CatalogError, InvalidSceneTitle, QuotationRestylingTaskError, StoreError};

#[derive(Debug, Error)]
pub enum ScenesError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Untitled(#[from] InvalidSceneTitle),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Tasks(#[from] QuotationRestylingTaskError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for ScenesError {
    fn lost_race(&self) -> bool {
        matches!(self, ScenesError::Transaction(failure) if failure.lost_race())
    }
}
