use eventsourcing::ServiceError;
use ids::InvalidId;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;
use crate::notice::NoticeError;

#[derive(Debug, Error)]
pub enum NotificationsError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Events(#[from] ServiceError<NoticeError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for NotificationsError {
    fn lost_race(&self) -> bool {
        matches!(self, NotificationsError::Events(failure) if failure.lost_race())
    }
}
