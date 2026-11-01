use authorizing::AuthorizerError;
use thiserror::Error;
use transacting::Failure;

use crate::catalog::CatalogError;

#[derive(Debug, Error)]
pub enum AppearancesError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such idea")]
    IdeaNotThere,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}
