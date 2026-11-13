use access::AccessError;
use authorizing::AuthorizerError;
use belonging::BelongingsError;
use eventsourcing::ServiceError;
use ids::InvalidId;
use registry::RegistryError;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::board::BoardError;
use crate::catalog::CatalogError;

#[derive(Debug, Error)]
pub enum BoardsError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<BoardError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    Access(#[from] AccessError),
    #[error(transparent)]
    Belongings(#[from] BelongingsError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("there is no such idea in the board's project")]
    IdeaNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for BoardsError {
    fn lost_race(&self) -> bool {
        matches!(self, BoardsError::Events(failure) if failure.lost_race())
    }
}
