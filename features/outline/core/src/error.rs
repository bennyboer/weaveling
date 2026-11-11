use authorizing::AuthorizerError;
use belonging::BelongingsError;
use eventsourcing::ServiceError;
use ids::InvalidId;
use registry::RegistryError;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;
use crate::outline::OutlineError;

#[derive(Debug, Error)]
pub enum OutlinesError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<OutlineError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    Belongings(#[from] BelongingsError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("there is no such idea or scene in the outline's project")]
    AttachmentNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for OutlinesError {
    fn lost_race(&self) -> bool {
        matches!(self, OutlinesError::Events(failure) if failure.lost_race())
    }
}
