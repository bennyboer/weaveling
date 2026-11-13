use access::AccessError;
use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use ids::InvalidId;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::asset::AssetError;
use crate::catalog::CatalogError;
use crate::contents::ContentsError;
use crate::images::ImageRefused;
use crate::limits::Limit;

#[derive(Debug, Error)]
pub enum AssetsError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<AssetError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Contents(#[from] ContentsError),
    #[error(transparent)]
    Image(#[from] ImageRefused),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    Access(#[from] AccessError),
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("reading the project is all this account may do in it")]
    NotAnAuthor,
    #[error("only an account keeps assets of its own")]
    NoAccount,
    #[error("only an operator gives an account a limit of its own")]
    NotAnOperator,
    #[error("a limit of more than can be counted")]
    LimitUncountable,
    #[error("the upload would pass a limit on what may be kept")]
    LimitReached(Limit),
    #[error("the asset is told kept, yet the bytes for it are not there")]
    ContentsMissing,
}

impl LostRace for AssetsError {
    fn lost_race(&self) -> bool {
        match self {
            AssetsError::Events(failure) => failure.lost_race(),
            AssetsError::Transaction(failure) => failure.lost_race(),
            _ => false,
        }
    }
}
