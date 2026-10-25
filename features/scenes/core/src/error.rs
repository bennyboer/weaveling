use authorizing::AuthorizerError;
use ids::InvalidId;
use thiserror::Error;

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
}
