use access::AccessError;
use authorizing::AuthorizerError;
use eventsourcing::ServiceError;
use thiserror::Error;
use transacting::{Failure, LostRace};

use crate::catalog::CatalogError;
use crate::comment::InvalidComment;
use crate::piece::FeedbackRefused;

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
    #[error("the reading copy is not known to the feedback yet")]
    ReadingCopyUnknown,
    #[error("there is no such project")]
    ProjectNotThere,
    #[error(
        "feedback is asked for in a project or on some reading copies, so it is known whose it is"
    )]
    ProjectUnnamed,
    #[error("only the authors and owners of a project mark its feedback done")]
    MarkedByAuthorsAlone,
    #[error("a reply is never marked done, as being done stays with the comment it replies to")]
    ReplyNotMarked,
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error(transparent)]
    Access(#[from] AccessError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("the transaction around the change failed")]
    Transaction(#[from] Failure),
    #[error(transparent)]
    Events(ServiceError<FeedbackRefused>),
}

/// The aggregate's refusals as the feedback feature tells them: a piece deleted, not there yet or not yet given is not there.
impl From<ServiceError<FeedbackRefused>> for FeedbackError {
    fn from(failure: ServiceError<FeedbackRefused>) -> Self {
        match failure {
            ServiceError::Refused(FeedbackRefused::NotYours) => FeedbackError::NotYours,
            ServiceError::Refused(FeedbackRefused::PassageEmpty) => FeedbackError::PassageEmpty,
            ServiceError::Refused(FeedbackRefused::ReplyNotMarked) => FeedbackError::ReplyNotMarked,
            ServiceError::Refused(
                FeedbackRefused::NotSubmitted
                | FeedbackRefused::Deleted
                | FeedbackRefused::NotDraftedYet,
            )
            | ServiceError::NotFound { .. } => FeedbackError::FeedbackNotThere,
            other => FeedbackError::Events(other),
        }
    }
}

impl LostRace for FeedbackError {
    fn lost_race(&self) -> bool {
        match self {
            FeedbackError::Transaction(failure) => failure.lost_race(),
            FeedbackError::Events(failure) => failure.lost_race(),
            _ => false,
        }
    }
}
