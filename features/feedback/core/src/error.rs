use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;
use crate::comment::InvalidComment;

#[derive(Debug, Error)]
pub enum FeedbackError {
    #[error(transparent)]
    InvalidComment(#[from] InvalidComment),
    #[error("a passage holds at least one character of one block, and quotes it")]
    PassageEmpty,
    #[error("there is no such feedback")]
    FeedbackNotThere,
    #[error("only whoever wrote a piece of feedback may change it")]
    NotYours,
    #[error("at most {} are asked for at once", crate::feedback::COUNTED_AT_ONCE)]
    TooManyCounted,
    #[error("the reading copy is closed for feedback")]
    ReadingCopyClosed,
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
}

impl LostRace for FeedbackError {
    fn lost_race(&self) -> bool {
        match self {
            FeedbackError::Transaction(failure) => failure.lost_race(),
            _ => false,
        }
    }
}
