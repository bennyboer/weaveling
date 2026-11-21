use thiserror::Error;

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
}
