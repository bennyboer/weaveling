use authorizing::AuthorizerError;
use thiserror::Error;

use crate::catalog::CatalogError;

#[derive(Debug, Error)]
pub enum AppearancesError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such idea")]
    IdeaNotThere,
}
