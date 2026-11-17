use thiserror::Error;

use crate::catalog::CatalogError;

#[derive(Debug, Error)]
pub enum ReadingsError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("there is no such project")]
    ProjectNotThere,
}
